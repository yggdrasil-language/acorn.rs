use std::io::{Cursor, Write};

use acorn_cfb::CompoundFile;
use cfb::CompoundFile as OracleCompoundFile;

fn build_oracle_fixture() -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut compound = OracleCompoundFile::create(&mut buffer).expect("create compound file");
        {
            let mut stream = compound
                .create_stream("/WordDocument")
                .expect("create WordDocument");
            stream
                .write_all(b"Hello WordDocument")
                .expect("write WordDocument");
        }
        {
            let mut stream = compound.create_stream("/0Table").expect("create 0Table");
            stream.write_all(b"table-bytes").expect("write 0Table");
        }
        {
            let mut stream = compound.create_stream("/MiniOnly").expect("create mini stream");
            stream.write_all(b"tiny").expect("write mini stream");
        }
    }
    buffer.into_inner()
}

#[test]
fn oracle_matches_third_party_cfb_streams() {
    let bytes = build_oracle_fixture();
    let mut oracle = OracleCompoundFile::open(Cursor::new(bytes.clone())).expect("open oracle");
    let reader = CompoundFile::open(bytes).expect("open acorn-cfb");

    for path in ["/WordDocument", "/0Table", "/MiniOnly"] {
        let mut oracle_stream = oracle
            .open_stream(path)
            .unwrap_or_else(|error| panic!("oracle open {path}: {error}"));
        let mut expected = Vec::new();
        oracle_stream
            .read_to_end(&mut expected)
            .unwrap_or_else(|error| panic!("oracle read {path}: {error}"));

        let actual = reader
            .read_stream(path)
            .unwrap_or_else(|error| panic!("acorn read {path}: {error}"));
        assert_eq!(actual, expected, "stream bytes differ for {path}");
    }
}

use std::io::Read;

#[test]
fn rejects_non_compound_input() {
    let error = CompoundFile::open([0u8; 512]).expect_err("expected error");
    assert!(
        matches!(
            error,
            acorn_cfb::CfbError::InvalidSignature
                | acorn_cfb::CfbError::InvalidSectorShift { .. }
                | acorn_cfb::CfbError::InvalidMiniSectorShift { .. }
        ),
        "unexpected error: {error}"
    );
}
