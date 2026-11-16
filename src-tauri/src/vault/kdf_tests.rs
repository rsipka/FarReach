use super::*;

#[test]
fn test_derive_kek_deterministic() {
    let password = b"test password";
    let salt = [0xAB; 32];

    let kek1 = derive_kek(password, &salt).unwrap();
    let kek2 = derive_kek(password, &salt).unwrap();

    assert!(kek1.with_key(|a| kek2.with_key(|b| a == b)));
}

#[test]
fn test_derive_kek_different_salts() {
    let password = b"test password";
    let salt1 = [0xAB; 32];
    let salt2 = [0xCD; 32];

    let kek1 = derive_kek(password, &salt1).unwrap();
    let kek2 = derive_kek(password, &salt2).unwrap();

    assert!(!kek1.with_key(|a| kek2.with_key(|b| a == b)));
}
