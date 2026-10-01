use linkle::format::nxo::NxoFile;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 3, "input ELF, output NRO");
    NxoFile::from_elf(&args[1]).expect("parse ELF")
        .write_nro(&mut std::fs::File::create(&args[2]).expect("create NRO"), None, None, None)
        .expect("write NRO");
}
