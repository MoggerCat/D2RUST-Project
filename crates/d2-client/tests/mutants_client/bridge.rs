// Spec: specs/client/bridge.md
//! Mutation-testing gaps (METHODS M08) of the bridge's plain-Rust part
//! (`docs/handoff/mutants-client.md`).

use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_client::bridge::{Bridge, BridgeError};
use d2_proto::PROTOCOL_VERSION;

struct VersionLink(u32);

impl ServerLink for VersionLink {
    fn protocol_version(&self) -> u32 {
        self.0
    }

    fn send(&mut self, _: SendQueue, _: &[u8]) -> Result<Sent, LinkError> {
        Ok(Sent::Queued)
    }

    fn pump(&mut self) -> Result<Pumped, LinkError> {
        Ok(Pumped::default())
    }

    fn receive(&mut self) -> Vec<Vec<u8>> {
        Vec::new()
    }
}

/// §9 rule 1 through a boxed link (how the app holds its link): the
/// version the box reports is the inner link's, so a foreign version is
/// still refused.
// Covers: specs/client/bridge.md §9 r1
#[test]
fn boxed_link_reports_the_inner_version() {
    let other = PROTOCOL_VERSION + 1;
    let boxed: Box<dyn ServerLink> = Box::new(VersionLink(other));
    assert_eq!(boxed.protocol_version(), other);
    match Bridge::new(boxed) {
        Err(BridgeError::Version { client, server }) => {
            assert_eq!((client, server), (PROTOCOL_VERSION, other));
        }
        Err(e) => panic!("wrong error {e}"),
        Ok(_) => panic!("foreign protocol version accepted"),
    }
    let ours: Box<dyn ServerLink> = Box::new(VersionLink(PROTOCOL_VERSION));
    assert!(Bridge::new(ours).is_ok());
}

/// §6 rule 1: every row names an owner (`TBD` or a spec) and a name; an
/// empty owner alone is malformed, as is an empty name alone.
#[test]
fn dispatch_tsv_refuses_an_empty_owner_or_name() {
    use d2_client::bridge::dispatch::{parse, TsvError, TSV};

    assert!(parse(TSV).is_ok());
    let row = |l: &str| l.starts_with("0x05\t");
    let no_owner: String = TSV
        .lines()
        .map(|l| {
            if row(l) {
                let (head, _) = l.rsplit_once('\t').unwrap();
                format!("{head}\t\n")
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    assert_eq!(parse(&no_owner), Err(TsvError::Empty { line: 7 }));
    let no_name: String = TSV
        .lines()
        .map(|l| {
            if row(l) {
                let (_, owner) = l.rsplit_once('\t').unwrap();
                format!("0x05\t\t{owner}\n")
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    assert_eq!(parse(&no_name), Err(TsvError::Empty { line: 7 }));
}

/// §6 rule 1: ids end at 0xB4; a table running past id 0xFF (which no
/// `0xNN` id can name) is refused with its row count.
#[test]
fn dispatch_tsv_past_id_ff_reports_its_row_count() {
    use d2_client::bridge::dispatch::{parse, TsvError};

    let mut text = String::from("id\tname\towner\n");
    for id in 0..=0xFFu32 {
        text.push_str(&format!("0x{id:02X}\tm\tTBD\n"));
    }
    text.push_str("0x100\tm\tTBD\n");
    assert_eq!(parse(&text), Err(TsvError::Rows(257)));
}

fn no_op(
    _: &mut d2_client::bridge::world::ClientWorld,
    _: &d2_client::bridge::dispatch::Message<'_>,
) -> Result<(), d2_client::bridge::dispatch::HandlerError> {
    Ok(())
}

/// §6 rule 5: two handlers for one id are reported; a handler whose owner
/// equals its row's owner is not a mismatch.
#[test]
fn dispatch_check_duplicate_handler_and_matching_owner() {
    use d2_client::bridge::dispatch::{check, parse, Handler, Mismatch, TSV};

    // Rows with no owner but 0x61 (a general id, no unit handler).
    let mut rows = parse(TSV).unwrap();
    for r in &mut rows {
        r.owner = None;
    }
    rows[0x61].owner = Some("specs/x.md".into());
    let h = Handler {
        id: 0x61,
        owner: "specs/x.md",
        handle: d2_client::bridge::dispatch::Handle::General(no_op),
    };
    assert_eq!(check(&rows, &[h]), Vec::new());
    assert_eq!(
        check(&rows, &[h, h]),
        vec![Mismatch::BadHandler { id: 0x61 }]
    );
}

/// §6 rule 3: unowned messages are counted per id in the receive log.
#[test]
fn unowned_messages_are_counted_per_id() {
    let mut bridge = Bridge::new(VersionLink(PROTOCOL_VERSION)).unwrap();
    // 0x61 (2 bytes, no owner spec) twice, then 0x5F (5 bytes) once.
    bridge
        .receive_chunk(&[0x61, 0x07, 0x61, 0x07, 0x5F, 1, 2, 3, 4])
        .unwrap();
    let counts: Vec<(u8, u64)> = bridge
        .log()
        .unowned
        .iter()
        .map(|(&id, &n)| (id, n))
        .collect();
    assert_eq!(counts, vec![(0x5F, 1), (0x61, 2)]);
}

/// §6 rules 2 and 4: messages a registered handler applies are counted as
/// handled in the receive log, across chunks.
#[test]
fn handled_messages_are_counted_in_the_log() {
    use d2_client::bridge::dispatch::Dispatch;

    let mut dispatch = Dispatch::empty();
    dispatch.set(0x1A, "specs/x.md", no_op);
    let mut bridge = Bridge::with_dispatch(VersionLink(PROTOCOL_VERSION), dispatch).unwrap();
    let report = bridge.receive_chunk(&[0x1A, 0x07, 0x1A, 0x07]).unwrap();
    assert_eq!(report.handled, 2);
    bridge.receive_chunk(&[0x1A, 0x07]).unwrap();
    assert_eq!(bridge.log().handled, 3);
    assert!(bridge.log().unowned.is_empty());
}
