// Spec: specs/world/hirelings.md §13; specs/sim/pets.md §8; specs/client/model.md §14 (server builder → client handler)
//! HANDOFF §7 "Ninth set" item I-1: S→C 0x7A PetAction as the server
//! builds it (`d2_sim::world::hirelings::pets::pet_action`, the encoder
//! of every 0x7A the server sends: the pet-add broadcast without seed and
//! name, and the remove broadcasts of `hirelings.md` §5 rule 5, §6 rule 4,
//! §8 rule 2) read by the client's handler (`0x0045E860`, the bridge's
//! dispatch table from the spec).
//!
//! The server writes the pet GUID @5 and the owner GUID @9
//! (`hirelings.md` §13 rule 2, the confirmed `server-messages.tsv` row);
//! the client reads the owner @5 and the pet @9 (`pets.md` §8,
//! `model.md` §14 rule 2). The test sends one add and checks that the
//! client's pet record holds the pet and owner the server meant: it fails
//! until the spec owner settles the layout (PC 1, item I-1) and one side
//! changes. No code is changed here.
use d2_client::bridge::dispatch::{Dispatch, Handle, Message};
use d2_client::bridge::world::{ClientWorld, ModelInputs, PET_HIRELING};
use d2_sim::world::hirelings::pets::{pet_action, ACTION_ADD, ACTION_REMOVE};

const PET: u32 = 0x21;
const OWNER: u32 = 0x05;
const CLASS: u16 = 0x14F;

fn receive(w: &mut ClientWorld, bytes: &[u8]) {
    let d = Dispatch::from_spec().expect("the spec's dispatch table");
    let entry = d.get(bytes[0]).expect("0x7A has a client handler");
    let Handle::General(handle) = entry.handle else {
        panic!("0x7A is a general handler");
    };
    let inputs = ModelInputs::default();
    let msg = Message {
        id: bytes[0],
        bytes,
        unit: None,
        inputs: &inputs,
    };
    handle(w, &msg).expect("handled");
}

// Covers: specs/world/hirelings.md §13 r2; specs/sim/pets.md §8; specs/client/model.md §14 r2
#[test]
#[ignore = "I-1: 0x7A layout disagreement, waiting on PC 1"]
fn server_0x7a_reaches_the_client_with_the_same_pet_and_owner() {
    let mut w = ClientWorld::default();
    // The add of a hireling (pet type 7) of class 0x14F.
    let add = pet_action(ACTION_ADD, PET_HIRELING, CLASS, PET, OWNER);
    assert_eq!(add.len(), 13);
    receive(&mut w, &add);
    assert_eq!(w.pets.len(), 1);
    let r = &w.pets[0];
    assert_eq!((r.pet_type, r.class), (PET_HIRELING, CLASS));
    assert_eq!(
        (r.pet, r.owner),
        (PET, OWNER),
        "the client read the server's pet GUID as the owner and the owner as the pet"
    );
    // The remove broadcast carries only the GUID (§13 rule 2): the client
    // must find the record by that pet GUID.
    let remove = pet_action(ACTION_REMOVE, 0, 0, PET, 0);
    receive(&mut w, &remove);
    assert!(
        w.pets.is_empty(),
        "the remove found no record: {:?}",
        w.pets
    );
}
