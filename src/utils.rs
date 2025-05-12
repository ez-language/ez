pub fn read_source_file(path: &str) -> Result<String, std::io::Error> {
    std::fs::read_to_string(path)
}
