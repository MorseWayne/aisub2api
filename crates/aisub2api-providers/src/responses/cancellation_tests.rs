use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{Duration, timeout};

#[tokio::test]
async fn dropping_responses_consumer_closes_idle_upstream_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let upstream = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 1024];
        while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            let length = socket.read(&mut buffer).await.unwrap();
            assert_ne!(length, 0);
            request.extend_from_slice(&buffer[..length]);
        }
        let event = "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-cancel\",\"output\":[]}}\n\n";
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n{:x}\r\n{event}\r\n",
            event.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
        socket.read(&mut buffer).await.unwrap()
    });
    let response = reqwest::Client::new()
        .get(format!("http://{address}/"))
        .send()
        .await
        .unwrap();
    let mut stream =
        stream_native_responses_response_with_provider_observer(response, None, 1024 * 1024);
    let event = timeout(Duration::from_secs(2), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(event.native_event().is_some());
    drop(stream);
    assert_eq!(
        timeout(Duration::from_secs(2), upstream)
            .await
            .expect("dropping the consumer must close an idle upstream")
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn incomplete_terminal_event_closes_sse_and_records_usage() {
    for native in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let terminal = json!({"type":"response.incomplete","response":{
            "id":"resp-incomplete", "model":"gpt-6.1-sol", "status":"incomplete", "output":[],
            "incomplete_details":{"reason":"budget_exhausted"},
            "usage":{"input_tokens":10,"output_tokens":7,"output_tokens_details":{"reasoning_tokens":4}}
        }});
        let expected = terminal.clone();
        let upstream = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0; 1024];
            let mut request = Vec::new();
            while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let length = socket.read(&mut buffer).await.unwrap();
                assert_ne!(length, 0);
                request.extend_from_slice(&buffer[..length]);
            }
            let event = format!("data: {terminal}\n\n");
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n{:x}\r\n{event}\r\n",
                event.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            socket.read(&mut buffer).await.unwrap()
        });
        let response = reqwest::Client::new()
            .get(format!("http://{address}/"))
            .send()
            .await
            .unwrap();
        let metadata = Arc::new(std::sync::Mutex::new(Vec::new()));
        let observed = Arc::clone(&metadata);
        let observer: ProviderRequestObserver = Arc::new(move |event| {
            if let Some(value) = event.stream_metadata {
                observed.lock().unwrap().push(value);
            }
        });
        let mut stream = if native {
            stream_native_responses_response_with_provider_observer(
                response,
                Some(observer),
                1024 * 1024,
            )
        } else {
            stream_responses_response_with_context_and_provider_observer(
                response,
                ReasoningMarkerMode::default(),
                ResponsesCorrelation::default(),
                Some(observer),
                1024 * 1024,
            )
        };
        let result = timeout(Duration::from_secs(2), stream.next())
            .await
            .unwrap()
            .unwrap();
        if native {
            let result = result.unwrap();
            let Some(crate::provider::NativeProviderEvent::OpenAiResponses(event)) =
                result.native_event()
            else {
                panic!("missing native terminal event");
            };
            assert_eq!(event.data, expected);
        } else {
            assert!(result.is_err());
        }
        assert!(
            timeout(Duration::from_secs(2), stream.next())
                .await
                .unwrap()
                .is_none()
        );
        {
            let metadata = metadata.lock().unwrap();
            assert_eq!(metadata[0].request_id.as_deref(), Some("resp-incomplete"));
            assert_eq!(metadata[0].usage.as_ref().unwrap().output_tokens, 7);
            assert_eq!(
                metadata[0].usage.as_ref().unwrap().reasoning_output_tokens,
                4
            );
        }
        assert_eq!(
            timeout(Duration::from_secs(2), upstream)
                .await
                .unwrap()
                .unwrap(),
            0
        );
    }
}
