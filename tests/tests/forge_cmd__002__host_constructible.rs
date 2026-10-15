//! Host constructibility test for Cmd variants.
//!
//! This test verifies that all Cmd variants can be constructed and debug-printed on the host
//! (without the wasm feature), ensuring no web_sys types leak into the public Cmd shape.

use ui::app::cmd::{ApiError, Cmd, EffectKind, FileHandle, Method};

#[test]
fn all_cmd_variants_constructible_and_debuggable_on_host() {
    // A dummy message type for testing
    #[derive(Debug, Clone, PartialEq)]
    enum TestMsg {
        Done,
        Got(Result<Vec<u32>, ApiError>),
    }

    // A test endpoint
    struct Numbers;
    impl ui::app::cmd::Endpoint for Numbers {
        const METHOD: Method = Method::Get;
        type Response = Vec<u32>;
        fn path(&self) -> String {
            "/api/numbers".into()
        }
    }

    // A test file endpoint
    struct TestFileEndpoint;
    impl ui::app::cmd::FileEndpoint for TestFileEndpoint {
        fn path(&self) -> String {
            "/api/upload".into()
        }
    }

    // Test FileHandle construction on host
    let file_handle = FileHandle::for_host("test.pdf", 1024, "application/pdf");
    assert_eq!(file_handle.name(), "test.pdf");
    assert_eq!(file_handle.size(), 1024);
    assert_eq!(file_handle.mime_type(), "application/pdf");

    // Test all Cmd variants can be constructed and debug-printed
    let cmds: Vec<Cmd<TestMsg>> = vec![
        Cmd::None,
        Cmd::Batch(vec![]),
        Cmd::None, // Batch with None gets filtered out
        Cmd::Batch(vec![Cmd::None]),
        Cmd::request(Numbers, TestMsg::Got),
        Cmd::navigate("/test"),
        Cmd::load("/test"),
        Cmd::replace_path("/test"),
        Cmd::share_pdf("data:application/pdf;base64,test", "test.pdf", |_| {
            TestMsg::Done
        }),
        Cmd::listen(|_| TestMsg::Done),
        Cmd::preview_pdf("/api/preview", serde_json::json!({}), |_| TestMsg::Done),
        Cmd::render_signature("John Doe", 0, |_| TestMsg::Done),
        Cmd::storage_read("key", |_| TestMsg::Done),
        Cmd::storage_write("key", Some("value".to_string())),
        Cmd::after(100, TestMsg::Done),
        Cmd::upload(
            FileHandle::for_host("photo.jpg", 2048, "image/jpeg"),
            TestFileEndpoint,
            vec![],
            vec![],
            |_| TestMsg::Done,
        ),
        Cmd::video_upload(
            FileHandle::for_host("video.mp4", 50_000_000, "video/mp4"),
            "prop-123".into(),
            "gallery".into(),
            "A test video".into(),
            |_| TestMsg::Done,
            |_| TestMsg::Done,
        ),
        Cmd::post_form(
            TestFileEndpoint,
            vec![("field1".into(), "value1".into())],
            FileHandle::for_host("doc.pdf", 512, "application/pdf"),
            |_| TestMsg::Done,
        ),
        Cmd::init_map(
            "map-key",
            37.7749,
            -122.4194,
            "Test Location",
            "map-container",
            |_| TestMsg::Done,
        ),
    ];

    // Verify each command can be debug-printed and has the correct effect kind
    for cmd in &cmds {
        let debug_str = format!("{:?}", cmd);
        assert!(
            !debug_str.is_empty(),
            "Cmd variant {:?} produced empty debug output",
            cmd
        );

        // Verify effect_kind() works for all variants
        let _kind = cmd.effect_kind();
    }

    // Verify specific variants have expected debug output patterns
    let upload_cmd = Cmd::upload(
        FileHandle::for_host("photo.jpg", 2048, "image/jpeg"),
        TestFileEndpoint,
        vec![],
        vec![],
        |_| TestMsg::Done,
    );
    let upload_debug = format!("{:?}", upload_cmd);
    assert!(
        upload_debug.contains("Upload"),
        "Upload debug: {}",
        upload_debug
    );

    let video_cmd = Cmd::video_upload(
        FileHandle::for_host("video.mp4", 50_000_000, "video/mp4"),
        "prop-123".into(),
        "gallery".into(),
        "A test video".into(),
        |_| TestMsg::Done,
        |_| TestMsg::Done,
    );
    let video_debug = format!("{:?}", video_cmd);
    assert!(
        video_debug.contains("VideoUpload"),
        "VideoUpload debug: {}",
        video_debug
    );

    let post_form_cmd = Cmd::post_form(
        TestFileEndpoint,
        vec![("field1".into(), "value1".into())],
        FileHandle::for_host("doc.pdf", 512, "application/pdf"),
        |_| TestMsg::Done,
    );
    let post_form_debug = format!("{:?}", post_form_cmd);
    assert!(
        post_form_debug.contains("PostForm"),
        "PostForm debug: {}",
        post_form_debug
    );

    // Verify map() works with FileHandle variants
    let mapped = upload_cmd.map(|_| TestMsg::Done);
    let mapped_debug = format!("{:?}", mapped);
    assert!(
        !mapped_debug.is_empty(),
        "Mapped command debug output empty"
    );

    // Verify into_requests() works
    let requests = Cmd::request(Numbers, TestMsg::Got).into_requests();
    assert_eq!(requests.len(), 1);

    // Verify Batch with multiple commands
    let batch = Cmd::Batch(vec![
        Cmd::request(Numbers, TestMsg::Got),
        Cmd::navigate("/test"),
    ]);
    let batch_debug = format!("{:?}", batch);
    assert!(batch_debug.contains("Request"));
    assert!(batch_debug.contains("Navigate"));
}

#[test]
fn cmd_effect_kind_covers_all_variants() {
    // This test ensures EffectKind enum covers all Cmd variants
    // by checking each variant can be classified
    #[derive(Debug, Clone, PartialEq)]
    enum TestMsg {
        Done,
    }

    struct Numbers;
    impl ui::app::cmd::Endpoint for Numbers {
        const METHOD: Method = Method::Get;
        type Response = Vec<u32>;
        fn path(&self) -> String {
            "/api/numbers".into()
        }
    }

    struct TestFileEndpoint;
    impl ui::app::cmd::FileEndpoint for TestFileEndpoint {
        fn path(&self) -> String {
            "/api/upload".into()
        }
    }

    let test_cases = vec![
        (Cmd::None, EffectKind::None),
        (Cmd::Batch(vec![]), EffectKind::Batch),
        (
            Cmd::request(Numbers, |_| TestMsg::Done),
            EffectKind::Request,
        ),
        (Cmd::navigate("/test"), EffectKind::Navigate),
        (Cmd::load("/test"), EffectKind::Load),
        (Cmd::replace_path("/test"), EffectKind::ReplacePath),
        (
            Cmd::share_pdf("data:...", "test.pdf", |_| TestMsg::Done),
            EffectKind::SharePdf,
        ),
        (Cmd::listen(|_| TestMsg::Done), EffectKind::Listen),
        (
            Cmd::render_signature("test", 0, |_| TestMsg::Done),
            EffectKind::RenderSignature,
        ),
        (
            Cmd::preview_pdf("/api/preview", serde_json::json!({}), |_| TestMsg::Done),
            EffectKind::PreviewPdf,
        ),
        (
            Cmd::storage_read("key", |_| TestMsg::Done),
            EffectKind::StorageRead,
        ),
        (
            Cmd::storage_write("key", Some("value".to_string())),
            EffectKind::StorageWrite,
        ),
        (Cmd::after(100, TestMsg::Done), EffectKind::After),
        (
            Cmd::upload(
                FileHandle::for_host("test.jpg", 100, "image/jpeg"),
                TestFileEndpoint,
                vec![],
                vec![],
                |_| TestMsg::Done,
            ),
            EffectKind::Upload,
        ),
        (
            Cmd::video_upload(
                FileHandle::for_host("test.mp4", 1000, "video/mp4"),
                "prop".into(),
                "role".into(),
                "caption".into(),
                |_| TestMsg::Done,
                |_| TestMsg::Done,
            ),
            EffectKind::VideoUpload,
        ),
        (
            Cmd::post_form(
                TestFileEndpoint,
                vec![],
                FileHandle::for_host("test.pdf", 100, "application/pdf"),
                |_| TestMsg::Done,
            ),
            EffectKind::PostForm,
        ),
        (
            Cmd::init_map("key", 0.0, 0.0, "title", "container", |_| TestMsg::Done),
            EffectKind::InitMap,
        ),
    ];

    for (cmd, expected_kind) in test_cases {
        assert_eq!(
            cmd.effect_kind(),
            expected_kind,
            "EffectKind mismatch for {:?}: got {:?}, expected {:?}",
            cmd,
            cmd.effect_kind(),
            expected_kind
        );
    }
}
