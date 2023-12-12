use std::{path::{Path, PathBuf}, fs::{create_dir, create_dir_all, remove_dir_all, File}, time::SystemTime};
use lib::forcast;
use random_string::generate;
use std::io::Write;
use chrono::NaiveDateTime;
use std::time::Duration;
use std::process::Command;

const TEST_CASE_PATH: &str = "tests/case";


struct TestFile {
    name: String,
    size: usize,
    created: NaiveDateTime,
}

impl TestFile {
    fn new(name: &str, created: &str, size: usize) -> Self {        
        Self { 
            name: String::from(name), 
            size, 
            created: parse_date_time(created) 
        }
    }

    fn content(&self) -> String {
        generate(self.size, "UTF-8")
    }
}

#[test]
fn test(){
    let now: SystemTime = parse_system_time("2023-05-01 21:05:06");
    let path = init_test(vec![
        TestFile::new("2019/info.md",       "2019-06-01 21:05:06", 5),
        TestFile::new("2019/content.dat",   "2019-07-02 20:01:01", 600),
        TestFile::new("2022/info.md",       "2022-10-01 21:05:06", 7),
        TestFile::new("2022/content.dat",   "2022-11-02 20:01:01", 801)
    ]);
   
    let actual = forcast(path.as_path(), Some(now));

    let created_min = parse_system_time("2019-06-01 21:05:06");
    assert!(actual.stats.created_min >= created_min);
    assert!(actual.stats.created_min <= created_min + Duration::from_secs(1));
    assert_eq!(actual.stats.created_max, now);
    assert_eq!(ceil(actual.stats.duration), Duration::from_secs(123552000));
    assert_eq!(actual.stats.total_size, 1413);
    assert_eq!(actual.stats.total_count, 4);
    assert_eq!(actual.interval, Duration::from_secs(31557600));
    assert_eq!(actual.slope, 7.681192486120618e-6);
    assert_eq!(actual.history.len(), 4);
    assert_eq!(actual.history.get(0).unwrap().size, 605);
    assert_eq!(actual.history.get(1).unwrap().size, 605);
    assert_eq!(actual.history.get(2).unwrap().size, 605);
    assert_eq!(actual.history.get(3).unwrap().size, 1413);
    assert_eq!(actual.forecast.len(), 36);
    for i in 0..36 {
        assert_eq!(actual.forecast.get(i).unwrap().size, 1413 + (i as u64 +1)*242);
    }
}


fn init_test(files: Vec<TestFile>) -> PathBuf {   
    let path = create_root_dir();
    for file in files {
        create_file(file);
    }
    path.to_path_buf()
}

fn create_root_dir() -> PathBuf {
    let path: &Path = Path::new(TEST_CASE_PATH);
    if path.exists() {
        if let Err(err) = remove_dir_all(path) {
            panic!("could not remove root test case directory {}", err)
        }
    }
    if let Err(err) = create_dir(path) {       
        panic!("could not create root test case directory {}", err)
    };
    path.to_path_buf()
}

fn create_file(test_file: TestFile){   
    let path = Path::new(TEST_CASE_PATH).join(&test_file.name);   

    let dir: &Path = match path.parent() {       
        None => panic!("could not resolve parent directory {}", path.display()),
        Some(parent) => parent
    };
    
    if let Err(err) = create_dir_all(dir) {       
        panic!("could not create file parent directory {}: {}", dir.display(), err)
    };

    let now = SystemTime::now();
    set_date(test_file.created);

    let mut file = match File::create(&path) {
        Err(err) => panic!("could not create file {}: {}", path.display(), err),
        Ok(file) => file,
    };

    if let Err(err) = file.write_all(test_file.content().as_bytes()) {
        panic!("could not write to file {}: {}", path.display(), err)
    }
    set_date(conv(now));
}


#[cfg(target_os = "linux")]
fn set_date(date: NaiveDateTime){
    let fmt_date = &date.format("%Y-%m-%d %H:%M:%S").to_string();            
    Command::new("sudo")
    .args(["date", "--utc", "--set", fmt_date])
    .output()
    .expect("Should set date");    
}

fn parse_date_time(value: &str) -> NaiveDateTime {
    match NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S") {
        Err(err) => panic!("couldn't prase created {}: {}", value, err),
        Ok(time) => time
    }
}

fn parse_system_time(value: &str) -> SystemTime {  
    let dt = parse_date_time(value);
    SystemTime::UNIX_EPOCH + Duration::new(dt.timestamp() as u64, dt.timestamp_subsec_nanos())
}

fn ceil(duration: Duration) -> Duration {
    Duration::from_secs((duration.as_millis() as f64 / 1000_f64).ceil() as u64)
}

fn conv(time: SystemTime) -> NaiveDateTime {
    match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(n) => NaiveDateTime::from_timestamp_millis(n.as_millis() as i64).unwrap(),
        Err(_) => panic!("SystemTime before UNIX EPOCH!"),
    }
}