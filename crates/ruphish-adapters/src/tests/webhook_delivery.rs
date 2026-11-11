use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn legacy_payload_signature_and_content_type_reach_receiver()
-> Result<(), Box<dyn std::error::Error>> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}/events", listener.local_addr()?);
    let body = br#"{"key1":"val1","key2":"val2","key3":"val3"}"#;
    let receiver = async {
        let (mut stream, _) = listener.accept().await?;
        let mut bytes = Vec::new();
        loop {
            let mut buffer = [0; 1024];
            let count = stream.read(&mut buffer).await?;
            if count == 0 {
                return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
            }
            bytes.extend_from_slice(&buffer[..count]);
            if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n")
                && bytes.len() >= end + 4 + body.len()
            {
                let headers = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                assert!(headers.starts_with("post /events http/1.1\r\n"));
                assert!(headers.contains("content-type: application/json\r\n"));
                assert!(headers.contains("x-gophish-signature: sha256=004b36ca3fcbc01a08b17bf5d4a7e1aa0b10e14f55f3f8bd9acac0c7e8d2635d"));
                assert_eq!(&bytes[end + 4..], body);
                stream.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await?;
                return Ok::<_, std::io::Error>(());
            }
            assert!(bytes.len() < 8192);
        }
    };
    let policy = crate::outbound::Policy::new(&["127.0.0.1".into()])?;
    let (received, outcome) = tokio::join!(
        receiver,
        crate::integrations::webhook::send(&policy, &url, "secret456", body, "legacy-fixture")
    );
    received?;
    assert_eq!(outcome.status, Some(204));
    assert_eq!(outcome.code, "accepted");
    Ok(())
}

// Independent known vectors from the legacy Go test suite.
#[test]
fn legacy_signature_vectors() {
    assert_eq!(
        crate::integrations::webhook::signature("secret123", b"some payload456"),
        Ok("sha256=ab7844c1e9149f8dc976c4188a72163c005930f3c2266a163ffe434230bdf761".into())
    );
    assert_eq!(
        crate::integrations::webhook::signature(
            "secret456",
            br#"{"key1":"val1","key2":"val2","key3":"val3"}"#
        ),
        Ok("sha256=004b36ca3fcbc01a08b17bf5d4a7e1aa0b10e14f55f3f8bd9acac0c7e8d2635d".into())
    );
}
