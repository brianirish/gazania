fn main() {
    match zbus::block_on(gazania_core::volumes::list_volumes()) {
        Ok(report) => {
            println!("source: {:?} {:?}", report.source, report.fallback_reason);
            println!("{}", serde_json::to_string_pretty(&report.drives).unwrap());
        }
        Err(e) => eprintln!("error: {e}"),
    }
}
