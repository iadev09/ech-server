use rustls_aws_lc_rs::hpke::ALL_SUPPORTED_SUITES;

fn encode_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let suite = ALL_SUPPORTED_SUITES[0];
    let (public_key, private_key) = suite.generate_key_pair()?;
    let s = suite.suite();

    let config_id = 0x42u8;
    let public_name = "localhost";
    let maximum_name_length = 64u8;

    let mut config = Vec::new();
    config.extend_from_slice(&0xfe0du16.to_be_bytes());
    let length_pos = config.len();
    config.extend_from_slice(&0u16.to_be_bytes());
    config.push(config_id);
    config.extend_from_slice(&s.kem.0.to_be_bytes());
    config.extend_from_slice(&(public_key.0.len() as u16).to_be_bytes());
    config.extend_from_slice(&public_key.0);
    config.extend_from_slice(&4u16.to_be_bytes());
    config.extend_from_slice(&s.sym.kdf_id.0.to_be_bytes());
    config.extend_from_slice(&s.sym.aead_id.0.to_be_bytes());
    config.push(maximum_name_length);
    config.push(public_name.len() as u8);
    config.extend_from_slice(public_name.as_bytes());
    config.extend_from_slice(&0u16.to_be_bytes());

    let contents_len = (config.len() - length_pos - 2) as u16;
    config[length_pos..length_pos + 2].copy_from_slice(&contents_len.to_be_bytes());

    let mut config_list = Vec::new();
    config_list.extend_from_slice(&(config.len() as u16).to_be_bytes());
    config_list.extend_from_slice(&config);

    println!("ECH_CONFIG_HEX={}", encode_hex(&config));
    println!("ECH_CONFIG_LIST_HEX={}", encode_hex(&config_list));
    println!(
        "ECH_PRIVATE_KEY_HEX={}",
        encode_hex(&private_key.secret_bytes())
    );

    Ok(())
}
