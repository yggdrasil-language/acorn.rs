use quick_xml::events::Event;
use quick_xml::Reader;

/// One `rootfile` entry from `META-INF/container.xml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerRootFile {
    pub full_path: String,
    pub media_type: String,
}

/// Parses EPUB container XML and returns discovered rootfiles.
pub fn parse_container_xml(xml: &[u8]) -> Result<Vec<ContainerRootFile>, String> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut rootfiles = Vec::new();
    while let Ok(event) = reader.read_event_into(&mut buf) {
        match event {
            Event::Start(tag) | Event::Empty(tag) => {
                if tag.local_name().as_ref() != b"rootfile" {
                    continue;
                }
                let mut full_path = None;
                let mut media_type = String::new();
                for attr in tag.attributes().flatten() {
                    match attr.key.local_name().as_ref() {
                        b"full-path" => {
                            full_path = attr
                                .unescape_value()
                                .ok()
                                .map(|value| value.into_owned());
                        }
                        b"media-type" => {
                            media_type = attr
                                .unescape_value()
                                .ok()
                                .map(|value| value.into_owned())
                                .unwrap_or_default();
                        }
                        _ => {}
                    }
                }
                if let Some(full_path) = full_path {
                    rootfiles.push(ContainerRootFile {
                        full_path,
                        media_type,
                    });
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    if rootfiles.is_empty() {
        return Err("container.xml has no rootfile".into());
    }
    Ok(rootfiles)
}
