use rcgen::{
    BasicConstraints, CertificateParams, CertifiedIssuer, DnType, ExtendedKeyUsagePurpose, IsCa,
    KeyPair, KeyUsagePurpose,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ca_key = KeyPair::generate()?;
    let mut ca_params = CertificateParams::default();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params
        .distinguished_name
        .push(DnType::CommonName, "ECH Test Root CA");
    ca_params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::CrlSign,
    ];
    let ca = CertifiedIssuer::self_signed(ca_params, ca_key)?;

    let leaf_key = KeyPair::generate()?;
    let mut leaf_params = CertificateParams::new(vec!["localhost".into()])?;
    leaf_params
        .distinguished_name
        .push(DnType::CommonName, "localhost");
    leaf_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    leaf_params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    let leaf = leaf_params.signed_by(&leaf_key, &ca)?;

    println!("CA_PEM_START");
    println!("{}", ca.pem());
    println!("CA_PEM_END");
    println!("CERT_PEM_START");
    println!("{}", leaf.pem());
    println!("CERT_PEM_END");
    println!("KEY_PEM_START");
    println!("{}", leaf_key.serialize_pem());
    println!("KEY_PEM_END");

    Ok(())
}
