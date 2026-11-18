//! Tests for the S3 sync backend, including Amazon's published SigV4 example.

use super::*;

fn token() -> String {
    serde_json::to_string(&S3Secret {
        access_key: "AK".into(),
        secret_key: "SK".into(),
    })
    .unwrap()
}

/// The GET example of Amazon's Signature Version 4 documentation.
#[test]
fn signs_as_amazon_documents() {
    let auth = authorization(
        "AKIAIOSFODNN7EXAMPLE",
        "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        "us-east-1",
        "GET",
        "/test.txt",
        &[
            ("host", "examplebucket.s3.amazonaws.com"),
            ("range", "bytes=0-9"),
            ("x-amz-content-sha256", EMPTY_SHA256),
            ("x-amz-date", "20130524T000000Z"),
        ],
        EMPTY_SHA256,
        "20130524T000000Z",
    );
    assert_eq!(
        auth,
        "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, \
         SignedHeaders=host;range;x-amz-content-sha256;x-amz-date, \
         Signature=f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41"
    );
}

#[test]
fn addresses_are_read() {
    let t = S3Target::parse(
        "s3://s3.eu-central-1.amazonaws.com/my-bucket/reach/vault",
        &token(),
    )
    .unwrap();
    assert_eq!(
        (t.bucket.as_str(), t.prefix.as_str(), t.region.as_str()),
        ("my-bucket", "reach/vault", "eu-central-1")
    );
    assert!(t.virtual_host);
    let (url, host, path) = t.object();
    assert_eq!(
        url,
        "https://my-bucket.s3.eu-central-1.amazonaws.com/reach/vault/reach-vault.reach"
    );
    assert_eq!(host, "my-bucket.s3.eu-central-1.amazonaws.com");
    assert_eq!(path, "/reach/vault/reach-vault.reach");

    let t = S3Target::parse("s3://s3.us-west-004.backblazeb2.com/vaults", &token()).unwrap();
    assert_eq!(
        (t.region.as_str(), t.virtual_host, t.prefix.as_str()),
        ("us-west-004", false, "")
    );
    assert_eq!(
        t.object().0,
        "https://s3.us-west-004.backblazeb2.com/vaults/reach-vault.reach"
    );

    let t = S3Target::parse("s3://acc.r2.cloudflarestorage.com/bk1", &token()).unwrap();
    assert_eq!(t.region, "auto");

    let t = S3Target::parse(
        "s3+http://localhost:9000/reach/my%20folder?region=eu-1&style=path",
        &token(),
    )
    .unwrap();
    assert_eq!(t.endpoint, "http://localhost:9000");
    assert_eq!(
        (t.region.as_str(), t.prefix.as_str()),
        ("eu-1", "my folder")
    );
    assert_eq!(
        t.object(),
        (
            "http://localhost:9000/reach/my%20folder/reach-vault.reach".into(),
            "localhost:9000".into(),
            "/reach/my%20folder/reach-vault.reach".into()
        )
    );
    assert_eq!(
        S3Target::parse("s3://minio.lan/bkt", &token())
            .unwrap()
            .region,
        "us-east-1"
    );
}

#[test]
fn bad_addresses_and_keys_are_refused() {
    assert!(
        S3Target::parse("s3+http://nas.example.com/bucket", &token()).is_err(),
        "keys in the clear over the network"
    );
    assert!(S3Target::parse("s3://s3.amazonaws.com/", &token()).is_err());
    assert!(S3Target::parse("s3://s3.amazonaws.com/a", &token()).is_err());
    assert!(S3Target::parse("s3://s3.amazonaws.com/bucket/../x", &token()).is_err());
    assert!(S3Target::parse("s3://s3.amazonaws.com/bucket?style=odd", &token()).is_err());
    assert!(S3Target::parse("s3://s3.amazonaws.com/bucket", "").is_err());
    assert!(S3Target::parse("s3://s3.amazonaws.com/bucket", r#"{"access_key":"a"}"#).is_err());
}

#[test]
fn error_answers_are_explained() {
    let t = S3Target::parse("s3://minio.lan/bkt", &token()).unwrap();
    let body =
        "<?xml version=\"1.0\"?><Error><Code>NoSuchBucket</Code><Message>x</Message></Error>";
    assert_eq!(error_code(body).as_deref(), Some("NoSuchBucket"));
    assert!(explain(reqwest::StatusCode::NOT_FOUND, body, &t)
        .to_string()
        .contains("bkt"));
    assert_eq!(region_of("s3-eu-west-1.amazonaws.com"), "eu-west-1");
    assert_eq!(
        region_of("s3.dualstack.ap-south-1.amazonaws.com"),
        "ap-south-1"
    );
    assert_eq!(region_of("s3.amazonaws.com"), "us-east-1");
}
