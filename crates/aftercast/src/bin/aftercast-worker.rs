fn main() {
    if let Err(error) = aftercast::run_worker() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
