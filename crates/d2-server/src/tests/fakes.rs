// Spec: specs/sim/intents-events.md
//! Test fakes for the seams. `TsvSizes` evaluates the size-rule grammar
//! of §5 over the rows of `client-messages.tsv` / `server-messages.tsv`
//! (stand-in for `d2-proto`); `FakeGame` stands in for `d2-sim`.

use std::collections::BTreeMap;

use crate::seams::*;

pub const CLIENT_TSV: &str = include_str!("../../../../specs/sim/client-messages.tsv");
pub const SERVER_TSV: &str = include_str!("../../../../specs/sim/server-messages.tsv");

/// Rows of a TSV as (header → cell) maps, in file order.
pub fn rows(tsv: &str) -> Vec<BTreeMap<String, String>> {
    let mut lines = tsv.lines();
    let header: Vec<&str> = lines.next().unwrap().split('\t').collect();
    lines
        .filter(|l| !l.is_empty())
        .map(|l| {
            header
                .iter()
                .zip(l.split('\t'))
                .map(|(h, c)| (h.to_string(), c.to_string()))
                .collect()
        })
        .collect()
}

pub fn hex(s: &str) -> u32 {
    u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap()
}

#[derive(Clone, Debug)]
pub enum Rule {
    Fixed(usize),
    Field {
        wide: bool,
        off: usize,
        mul: usize,
        add: usize,
        cap: Option<usize>,
        min: usize,
    },
    Chat,
    Chat26,
    Af,
}

pub fn parse_rule(s: &str) -> Rule {
    match s {
        "chat" => return Rule::Chat,
        "chat26" => return Rule::Chat26,
        "af" => return Rule::Af,
        _ => {}
    }
    if let Ok(n) = s.parse() {
        return Rule::Fixed(n);
    }
    let num = |v: &str| -> usize {
        if let Some(h) = v.strip_prefix("0x") {
            usize::from_str_radix(h, 16).unwrap()
        } else {
            v.parse().unwrap()
        }
    };
    let mut parts = s.split(';');
    let expr = parts.next().unwrap();
    let (mut cap, mut min) = (None, 0);
    for p in parts {
        let (k, v) = p.split_once('=').unwrap();
        match k {
            "cap" => cap = Some(num(v)),
            "min" => min = num(v),
            _ => panic!("rule option {k}"),
        }
    }
    let (ty, rest) = expr.split_once('@').unwrap();
    let wide = match ty {
        "u8" => false,
        "u16" => true,
        _ => panic!("rule type {ty}"),
    };
    let (mut off_s, mut mul, mut add) = (rest, 1, 0);
    if let Some((a, b)) = off_s.split_once('+') {
        off_s = a;
        add = num(b);
    }
    if let Some((a, b)) = off_s.split_once('*') {
        off_s = a;
        mul = num(b);
    }
    Rule::Field {
        wide,
        off: num(off_s),
        mul,
        add,
        cap,
        min,
    }
}

fn cstr(b: &[u8], at: usize) -> Result<usize, SizeError> {
    b.get(at..)
        .and_then(|s| s.iter().position(|&c| c == 0))
        .ok_or(SizeError::Incomplete)
}

pub fn eval(rule: &Rule, b: &[u8]) -> Result<usize, SizeError> {
    let need = |n: usize| {
        if b.len() >= n {
            Ok(())
        } else {
            Err(SizeError::Incomplete)
        }
    };
    match *rule {
        Rule::Fixed(0) => Err(SizeError::Invalid),
        Rule::Fixed(n) => Ok(n),
        Rule::Field {
            wide,
            off,
            mul,
            add,
            cap,
            min,
        } => {
            need(min.max(off + if wide { 2 } else { 1 }))?;
            let mut v = if wide {
                usize::from(u16::from_le_bytes([b[off], b[off + 1]]))
            } else {
                usize::from(b[off])
            };
            if cap.is_some_and(|c| v > c) {
                v = 0;
            }
            Ok(v * mul + add)
        }
        Rule::Chat => {
            need(3)?;
            let l1 = cstr(b, 3)?;
            let l2 = cstr(b, l1 + 4)?;
            need(l1 + l2 + 6)?;
            let c = b[l1 + l2 + 5] as i8 as isize;
            let size = (l1 + l2 + 6) as isize + c;
            // "needs >= size" is left to the classifier's "> the given
            // size" test (same result 3), as in `check_packets.py`.
            usize::try_from(size).map_err(|_| SizeError::Incomplete)
        }
        Rule::Chat26 => {
            need(10)?;
            let a = cstr(b, 10)?;
            let c = cstr(b, 10 + a + 1)?;
            Ok(10 + a + 1 + c + 1)
        }
        Rule::Af => {
            need(2)?;
            Ok(if b[1] == 0 { 2 } else { usize::from(b[1]) + 1 })
        }
    }
}

/// Size rules read from both TSVs.
pub struct TsvSizes {
    pub client: Vec<Rule>,
    pub server: Vec<Rule>,
}

impl TsvSizes {
    pub fn new() -> Self {
        let load = |tsv, col: &str| -> Vec<Rule> {
            rows(tsv)
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    assert_eq!(hex(&r["id"]) as usize, i, "ids in order");
                    parse_rule(&r[col])
                })
                .collect()
        };
        Self {
            client: load(CLIENT_TSV, "transport_size"),
            server: load(SERVER_TSV, "size"),
        }
    }
}

impl MessageSizes for TsvSizes {
    fn client_size(&self, msg: &[u8]) -> Result<usize, SizeError> {
        let rule = self
            .client
            .get(usize::from(msg[0]))
            .ok_or(SizeError::Invalid)?;
        eval(rule, msg)
    }
    fn server_size(&self, msg: &[u8]) -> Result<usize, SizeError> {
        let rule = self
            .server
            .get(usize::from(msg[0]))
            .ok_or(SizeError::Invalid)?;
        eval(rule, msg)
    }
}

#[derive(Clone, Debug)]
pub struct FakePlayer {
    pub gate: PlayerGate,
    pub point: Option<PointState>,
}

/// A game: players by client, units by id, a client list, and logs of
/// handler calls and ticks.
#[derive(Default)]
pub struct FakeGame {
    pub frame: i32,
    pub players: BTreeMap<ClientId, Option<FakePlayer>>,
    pub units: BTreeMap<(u32, u32), UnitTarget>,
    pub client_list: Vec<ClientId>,
    pub handled: Vec<(ClientId, Vec<u8>, usize)>,
    pub resyncs: Vec<ClientId>,
    /// Messages each tick queues: (client, bytes).
    pub tick_out: Vec<(ClientId, Vec<u8>)>,
    /// Messages each handler call queues.
    pub handler_out: Vec<(ClientId, Vec<u8>)>,
}

pub const ALIVE: PlayerGate = PlayerGate {
    mode: 1,
    uninterruptable: false,
};

impl FakeGame {
    pub fn with_player(client: ClientId, gate: PlayerGate) -> Self {
        let mut g = Self::default();
        g.players.insert(
            client,
            Some(FakePlayer {
                gate,
                point: Some(PointState {
                    player: Pos { x: 100, y: 100 },
                    last_accept: 0,
                }),
            }),
        );
        g.client_list.push(client);
        g
    }
}

impl Intents for FakeGame {
    fn player(&self, client: ClientId) -> PlayerLookup {
        match self.players.get(&client) {
            None => PlayerLookup::NotInGame,
            Some(None) => PlayerLookup::NoPlayer,
            Some(Some(p)) => PlayerLookup::Player(p.gate),
        }
    }
    fn frame(&self) -> i32 {
        self.frame
    }
    fn point_state(&self, client: ClientId) -> Option<PointState> {
        self.players.get(&client)?.as_ref()?.point
    }
    fn set_point_accept(&mut self, client: ClientId, frame: i32) {
        let p = self.players.get_mut(&client).unwrap().as_mut().unwrap();
        p.point.as_mut().unwrap().last_accept = frame;
    }
    fn queue_resync(&mut self, client: ClientId, out: &mut dyn MessageSink) {
        self.resyncs.push(client);
        out.queue(client, &[0x15; 11]).unwrap();
    }
    fn unit_target(&self, _client: ClientId, unit_type: u32, unit_id: u32) -> UnitTarget {
        self.units
            .get(&(unit_type, unit_id))
            .copied()
            .unwrap_or(UnitTarget::Missing)
    }
    fn handle(
        &mut self,
        client: ClientId,
        msg: &[u8],
        size: usize,
        out: &mut dyn MessageSink,
    ) -> ResultCode {
        self.handled.push((client, msg.to_vec(), size));
        for (c, m) in &self.handler_out {
            out.queue(*c, m).unwrap();
        }
        ResultCode::Done
    }
    fn clients(&self) -> Vec<ClientId> {
        self.client_list.clone()
    }
}

impl Tick for FakeGame {
    fn tick(&mut self, out: &mut dyn MessageSink) {
        self.frame += 1;
        for (c, m) in &self.tick_out {
            out.queue(*c, m).unwrap();
        }
    }
}

/// Records system messages.
#[derive(Default)]
pub struct FakeSession {
    pub seen: Vec<(ClientId, Vec<u8>, usize)>,
}

impl SessionHandler for FakeSession {
    fn system_message(
        &mut self,
        client: ClientId,
        msg: &[u8],
        size: usize,
        _out: &mut dyn MessageSink,
    ) {
        self.seen.push((client, msg.to_vec(), size));
    }
}

/// A clock the test sets.
#[derive(Clone, Copy, Debug, Default)]
pub struct ManualClock(pub u32);

impl Clock for ManualClock {
    fn now_ms(&mut self) -> u32 {
        self.0
    }
}
