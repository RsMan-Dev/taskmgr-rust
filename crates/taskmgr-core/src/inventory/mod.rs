//! Hardware inventory as a tree, in the spirit of dmidecode and lshw:
//! firmware, board, processor, memory modules, graphics, storage with
//! partitions, network, displays, audio, USB and PCI. Read without root;
//! live readings attach to the nodes that carry a `live` key.

mod devices;
mod peripherals;
mod platform;
mod processor;

use serde::Serialize;

use crate::{Root, Snapshot, SystemInfo};

#[derive(Clone, Debug, Default, Serialize)]
pub struct Node {
    /// English for fixed labels (translated by the interface), or a name.
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// A quantity the interface formats: with `unit` bytes, mhz, celsius...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
    /// Key of the live readings shown under this node (cpu, gpu:<slot>...).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live: Option<String>,
}

impl Node {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            ..Self::default()
        }
    }

    /// Adds `label: value` when the value is known and meaningful.
    pub fn text(&mut self, label: &str, value: Option<impl Into<String>>) -> &mut Self {
        if let Some(v) = value.map(Into::into).map(|v| v.trim().to_owned())
            && !v.is_empty()
            && !is_placeholder(&v)
        {
            self.children.push(Node {
                label: label.to_owned(),
                text: Some(v),
                ..Node::default()
            });
        }
        self
    }

    pub fn number(&mut self, label: &str, value: Option<f64>, unit: &'static str) -> &mut Self {
        if let Some(v) = value.filter(|v| v.is_finite()) {
            self.children.push(Node {
                label: label.to_owned(),
                number: Some(v),
                unit: Some(unit),
                ..Node::default()
            });
        }
        self
    }

    pub fn child(&mut self, node: Node) -> &mut Self {
        if !node.children.is_empty()
            || node.text.is_some()
            || node.number.is_some()
            || node.live.is_some()
        {
            self.children.push(node);
        }
        self
    }

    pub fn live(mut self, key: impl Into<String>) -> Self {
        self.live = Some(key.into());
        self
    }
}

/// Values firmware leaves when a field is not filled in.
fn is_placeholder(v: &str) -> bool {
    const EMPTY: [&str; 9] = [
        "Default string",
        "To Be Filled By O.E.M.",
        "To be filled by O.E.M.",
        "Not Specified",
        "Unknown",
        "None",
        "System Product Name",
        "System manufacturer",
        "x.x",
    ];
    // Serial numbers left as "00000000"; a single "0" is a real value.
    EMPTY.contains(&v) || (v.len() > 1 && v.chars().all(|c| c == '0' || c == ' '))
}

/// The whole tree. `snapshot` provides the devices currently present.
pub fn collect(root: &Root, snapshot: &Snapshot, info: &SystemInfo) -> Vec<Node> {
    let mut out = vec![
        platform::system(root, info),
        platform::board(root, info),
        processor::processor(root, &snapshot.cpu.info),
        platform::memory(root, snapshot),
    ];
    out.extend([
        devices::graphics(root, &snapshot.gpus),
        devices::storage(root, &snapshot.disks),
        devices::network(root, &snapshot.networks),
        peripherals::displays(root),
        peripherals::audio(root),
        peripherals::usb(root),
        peripherals::pci(root),
    ]);
    out.retain(|n| !n.children.is_empty());
    out
}
