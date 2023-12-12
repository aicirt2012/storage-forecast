use chrono::{offset::Utc, DateTime};
use core::cmp::min;
use std::process::exit;
use humansize::{format_size_i, DECIMAL};
use humantime::{format_duration, FormattedDuration};
use left_pad::leftpad;
use linreg::{linear_regression_of,Error};
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::time::{SystemTime,Duration};
use walkdir::WalkDir;


const YEAR: Duration = Duration::from_secs(31557600); // 365.25 days
const MONTH: Duration = Duration::from_secs(2629800); // 30.44 days
const DAY: Duration = Duration::from_secs(86400);
const HOUR: Duration = Duration::from_secs(3600);

struct File {
    size: u64,
    created: SystemTime,
}

impl File {
    fn new(created: SystemTime, size: u64) -> Self {
        Self{size, created}
    }
}

impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(created: {} size: {})", format_system_time_long(self.created), self.size)
    }
}

#[derive(PartialEq)]
#[derive(Debug)]
pub struct Group {
    pub size: u64,
    pub until: SystemTime,
}

impl Group {
    fn new(until: SystemTime, size: u64) -> Self {
        Self{size, until}
    }
}

impl fmt::Display for Group {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(until: {} size: {})", format_system_time(self.until), self.size)
    }
}

pub struct Stats {
    pub created_min: SystemTime,
    pub created_max: SystemTime,
    pub duration: Duration,
    pub total_size: u64,
    pub total_count: u64,
}

pub struct Forcast {
    pub stats: Stats,
    pub interval: Duration,
    pub slope: f64,
    pub history: Vec<Group>,
    pub forecast: Vec<Group>
}

pub fn forcast(path: &Path, now: Option<SystemTime>) -> Forcast {    
    assert_path(path);
    let files: Vec<File> = list_files(path); 
    let now: SystemTime = now.unwrap_or(SystemTime::now());
    let stats: Stats = statistics(&files, now);
    let interval: Duration = derive_interval(stats.duration);
    let history: Vec<Group>  = acc(group(&files, interval, now));
    let slope: f64 = regression(&history);
    let forecast: Vec<Group>  = estimate(slope, interval, now, stats.total_size);   
    
    print_stats(&stats, interval, slope);        
    print_list("History", &history);
    print_list("Forecast", &forecast);

    Forcast{
        stats,
        interval,
        slope,
        history,
        forecast
    }
}

fn assert_path(path: &Path) {
    if !path.exists() {
        println!("Path does not exist: {}", path.display());
        exit(1);
    }
}

fn list_files(path: &Path) -> Vec<File> {
    WalkDir::new(path)
    .into_iter()
    .filter_map(Result::ok)
    .filter(|entry| entry.file_type().is_file())
    .map(|file| file.metadata())
    .filter_map(Result::ok)
    .map(|metadata| File::new(metadata.created().unwrap(), metadata.len()))
    .collect()
}

fn statistics(files: &Vec<File>, max: SystemTime) -> Stats {
    let mut stats: Stats = Stats{
        created_min: SystemTime::now(),
        created_max: max,
        duration: Duration::from_secs(0),
        total_size: 0,
        total_count: files.len() as u64,
    };
    for file in files {
        stats.total_size += file.size;
        stats.created_min = min(stats.created_min, file.created);
    }  
    match stats.created_max.duration_since(stats.created_min) {
        Ok(duration) => { stats.duration = duration; }
        Err(e) => panic!("could not calculate duration between {:?}, {:?}: {}", stats.created_max, stats.created_min, e)
    }
    stats
}

fn derive_interval(duration: Duration) -> Duration {
    if duration >= YEAR * 3 {
        return YEAR;
    } else if duration >= MONTH * 3 {
        return MONTH;      
    } else if duration >= DAY * 3 {
        return DAY;      
    } else if duration >= HOUR * 3 {
        return HOUR;      
    }  
    panic!("An invteraval with less thatn 3 hours is not supported!")
}

fn group(files: &Vec<File>, interval: Duration, now: SystemTime) -> Vec<Group> {
    let absolut_min: SystemTime = files.iter().map(|f| f.created).min().unwrap();
    let mut map: HashMap<SystemTime, Group> = HashMap::new();   
    let mut max: SystemTime = now;
    let mut min: SystemTime = max.checked_sub(interval).unwrap();
    
    map.insert(max, Group::new(max, 0));
    while absolut_min.le(&min) {   
        min = min.checked_sub(interval).unwrap();
        max = max.checked_sub(interval).unwrap();
        map.insert(max, Group{
            until: max, 
            size: 0
        });
    }
   
    for file in files {
        let d = match now.duration_since(file.created) {
            Err(err) => panic!("could not calcuated duration {:?} {:?}: {}", file.created, now, err),
            Ok(duration) => duration
        };
        let x = d.as_secs() / interval.as_secs();
        let key = now.checked_sub(interval*x as u32).unwrap();
        map.entry(key).and_modify(|group| group.size += file.size);
    }

    let mut groups : Vec<Group> = map.into_values().collect();
    groups.sort_by_key(|g| g.until);
    groups
}


fn acc(groups: Vec<Group>) -> Vec<Group> {
    let mut acc_size: u64 = 0;
    groups.into_iter().map(|mut g| {
        acc_size += g.size;
        g.size = acc_size;
        g
    }).collect()
}

fn regression(history: &[Group]) -> f64 {
    let result: Vec<(f64, f64)> = history.iter().map(|g|{        
        match g.until.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(n) => (n.as_secs() as f64, g.size as f64),
            Err(_) => panic!("SystemTime before UNIX EPOCH!"),
        }        
    }).collect();
    let r: Result<(f64, f64), Error> = linear_regression_of(&result);
    r.unwrap().0
}

fn estimate(slope: f64, interval: Duration, now: SystemTime, size_now: u64) -> Vec<Group> {
    let mut results: Vec<Group> = Vec::new();   
    let mut min: SystemTime = now;
    let mut max: SystemTime = min.checked_add(interval).unwrap();
    let inerval_inc: u64  = (slope*(interval.as_secs() as f64)) as u64;
    let mut estimated_size: u64 = size_now;
    for _ in 0..36 {
        estimated_size += inerval_inc;
        results.push(Group{
            until: max,
            size: estimated_size
        });
        min = max;
        max = min.checked_add(interval).unwrap();
    } 
    results
}

fn print_list(label: &str, list: &[Group] ){
    println!("{}", format_list(label, list));
}

fn print_stats(stats: &Stats, interval: Duration, slope: f64) {
    println!("{}", format_stats(stats, interval, slope));
}

fn format_list(label: &str, groups: &[Group]) -> String {
    let mut s = format!("\n== {} ==\n", label);
    for group in groups {
        let size_label = format_size_i(group.size, DECIMAL);
        s += &format!("{}  {}\n", format_system_time(group.until), leftpad(size_label, 9));
    };
    s
}

fn format_stats(stats: &Stats, interval: Duration, slope: f64) -> String {
    format!("== Summary ==\n\
    Number of files:   {}\n\
    Size of files:     {}\n\
    Interval start:    {}\n\
    Interval end:      {}\n\
    Interval duration: {}\n\
    Group interval:    {}\n\
    Slope:             {}",
    stats.total_count,
    format_size_i(stats.total_size, DECIMAL),
    format_system_time(stats.created_min),
    format_system_time(stats.created_max),
    format_duration_trunc(stats.duration),
    format_duration(interval),
    format_slope(slope, interval))
}

fn format_system_time(time: SystemTime) -> String {
    let datetime: DateTime<Utc> = time.into();
    datetime.format("%Y.%m.%d").to_string()
}

fn format_system_time_long(time: SystemTime) -> String {
    let datetime: DateTime<Utc> = time.into();
    datetime.format("%Y.%m.%d %H:%M:%S").to_string()
}

fn format_duration_trunc(duration: Duration) -> FormattedDuration {
    format_duration(Duration::from_secs(duration.as_secs()))
}

fn format_slope(slope: f64, interval: Duration) -> String {
    let interval_inc = slope*(interval.as_secs() as f64);
    let human_slope = format_size_i(interval_inc, DECIMAL);
    let human_slope_interval = format_duration(interval).to_string()[1..].to_string(); 
    human_slope + "/" + &human_slope_interval
}


#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use super::*;

    #[test]
    fn statistics_test() {
        let files = vec![
            (File::new(init_system_time(2023, 2, 13), 10058)),
            (File::new(init_system_time(2018, 12, 31), 105)),
            (File::new(init_system_time(2020, 10, 1), 132)),
            (File::new(init_system_time(2020, 10, 1), 5068)),
            (File::new(init_system_time(2021, 7, 27), 9892)),
        ];
        let now = init_system_time(2023, 2, 28);
        let actual = statistics(&files, now);
        assert_eq!(actual.created_min, init_system_time(2018, 12, 31));
        assert_eq!(actual.created_max, now);
        assert_eq!(actual.duration, Duration::from_nanos(131328000000000000));
        assert_eq!(actual.total_size, 25255);
        assert_eq!(actual.total_count, 5);     
    }

    #[test]
    fn group_test() {
        let now = init_system_time(2023, 2, 28);
        let files = vec![
            (File::new(init_system_time(2019, 1, 26), 10)),
            (File::new(init_system_time(2021, 1, 26), 10)),
            (File::new(init_system_time(2022, 1, 26), 10)),
            (File::new(init_system_time(2022, 3, 1), 10)),
            (File::new(init_system_time(2023, 2, 13), 10058)),
        ];
        let expected = vec![
            (Group::new(now.checked_sub(YEAR*4).unwrap(), 10)),
            (Group::new(now.checked_sub(YEAR*3).unwrap(), 0)),
            (Group::new(now.checked_sub(YEAR*2).unwrap(), 10)),
            (Group::new(now.checked_sub(YEAR).unwrap(), 10)),
            (Group::new(now, 10068)),
        ];      
        let actual = group(&files, YEAR, now);
        assert_eq!(actual, expected);
    }

    #[test]
    fn acc_test() {
        let now = init_system_time(2023, 2, 28);
        let input = vec![
            (Group::new(now.checked_sub(YEAR*4).unwrap(), 40)),
            (Group::new(now.checked_sub(YEAR*3).unwrap(), 0)),
            (Group::new(now.checked_sub(YEAR*2).unwrap(), 30)),
            (Group::new(now.checked_sub(YEAR).unwrap(), 20)),
            (Group::new(now, 10)),
        ]; 
        let expected = vec![
            (Group::new(now.checked_sub(YEAR*4).unwrap(), 40)),
            (Group::new(now.checked_sub(YEAR*3).unwrap(), 40)),
            (Group::new(now.checked_sub(YEAR*2).unwrap(), 70)),
            (Group::new(now.checked_sub(YEAR).unwrap(), 90)),
            (Group::new(now, 100)),
        ]; 
        assert_eq!(expected, acc(input));
    }

    #[test]
    fn derive_interval_test() {
        assert_eq!(YEAR, derive_interval(Duration::from_secs(31557600*3)));
        assert_eq!(MONTH, derive_interval(Duration::from_secs(31557600*3-1)));
        assert_eq!(MONTH, derive_interval(Duration::from_secs(2629800*3)));
        assert_eq!(DAY, derive_interval(Duration::from_secs(2629800*3-1)));
        assert_eq!(DAY, derive_interval(Duration::from_secs(86400*3)));
        assert_eq!(HOUR, derive_interval(Duration::from_secs(86400*3-1)));
        assert_eq!(HOUR, derive_interval(Duration::from_secs(3600*3)));
    }

    #[test]
    #[should_panic]
    fn derive_interval_errors() {
        derive_interval(Duration::from_secs(3600*3-1));
    }

    #[test]
    fn regression_test() {
        let now = init_system_time(2023, 2, 28);
        let input = vec![
            (Group::new(now.checked_sub(YEAR*4).unwrap(), 1)),
            (Group::new(now.checked_sub(YEAR*3).unwrap(), 2)),
            (Group::new(now.checked_sub(YEAR*2).unwrap(), 4)),
            (Group::new(now.checked_sub(YEAR).unwrap(), 8)),
            (Group::new(now, 16)),
        ]; 
        let actual = regression(&input);
        assert_eq!(actual, 0.00000011407711613050423)
    }

    #[test]
    fn estimate_test() {
        let now = init_system_time(2023, 2, 28);
        let slope = 0.68;
        let inerval_inc: u64  = (slope*(YEAR.as_secs() as f64)) as u64;
        let expected: Vec<Group> = (1..37).map(|i|
            Group::new(now.checked_add(YEAR*i).unwrap(), 1+inerval_inc*i as u64)
        ).collect(); 
        let actual = estimate(slope, YEAR, now, 1);
        assert_eq!(actual, expected)
    }

    #[test]
    fn format_list_test() {
        let now = init_system_time(2023, 2, 28);
        let input = vec![
            (Group::new(now.checked_sub(YEAR*4).unwrap(), 40)),
            (Group::new(now.checked_sub(YEAR*3).unwrap(), 0)),
            (Group::new(now.checked_sub(YEAR*2).unwrap(), 30)),
            (Group::new(now.checked_sub(YEAR).unwrap(), 20)),
            (Group::new(now, 10)),
        ]; 
        let expected = "\n\
        == History ==\n\
        2019.02.28       40 B\n\
        2020.02.28        0 B\n\
        2021.02.27       30 B\n\
        2022.02.27       20 B\n\
        2023.02.28       10 B\n";

        let actual = format_list("History", &input );

        assert_eq!(actual, expected)
    }

    #[test]
    fn format_stats_test() {
        let stats = Stats{
            created_min: init_system_time(2019, 10, 25),
            created_max: init_system_time(2023, 7, 21),
            duration: Duration::from_secs(50000),
            total_size: 58454512121215,
            total_count: 218986,
        };

        let expected = "== Summary ==\n\
        Number of files:   218986\n\
        Size of files:     58.45 TB\n\
        Interval start:    2019.10.25\n\
        Interval end:      2023.07.21\n\
        Interval duration: 13h 53m 20s\n\
        Group interval:    1year\n\
        Slope:             21.46 MB/year";

        let actual = format_stats(&stats, YEAR, 0.68);

        assert_eq!(actual, expected)
    }

    #[test]
    fn format_system_time_test() {
        assert_eq!("2023.05.22", format_system_time(init_system_time(2023,5,22)));
    }

    #[test]
    fn format_duration_trunc_test() {
        let duration = Duration::from_millis(3601999);
        assert_eq!("1h 1s", format_duration_trunc(duration).to_string());
    }

    #[test]
    fn format_slope_test() {
        assert_eq!("28.40 MB/year", format_slope(0.9, YEAR));
        assert_eq!("47.34 MB/year", format_slope(1.5, YEAR));
    }

    fn init_system_time(year: i32, month: u32, day: u32) -> SystemTime {
        let day_time: DateTime<Utc> = Utc.with_ymd_and_hms(year, month, day, 0, 0, 0).unwrap();
        day_time.into()
    }
}