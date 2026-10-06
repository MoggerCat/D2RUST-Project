//! CI half of the GPU compositor (no GPU): buffer layouts, little-endian
//! packing, shader validity (naga), and the shader's algorithm run on the
//! packed bytes against the CPU reference, with perturbations (M08). The
//! byte-exact GPU run is the ignored `gpu_*` tests (local, needs a GPU).

use wgpu::naga;

use super::harness::{self, cases, diff, perturb, prepare, Case};
use super::pack::{self, emulate, GpuItem, Params, ITEM_SIZE, PARAMS_SIZE};
use super::*;
use crate::frames::AtlasSlot;
use crate::scene::{self, DrawItem, FrameImage, MapTable, Rect};

fn module() -> naga::Module {
    naga::front::wgsl::parse_str(SHADER).expect("WGSL parses")
}

fn cpu(case: &Case) -> Vec<u8> {
    scene::compose(&case.items, &case.frames, &case.maps, case.view).expect("valid case")
}

fn case_named(name: &str) -> Case {
    cases()
        .into_iter()
        .find(|c| c.name == name)
        .expect("case exists")
}

// Covers: specs/client/render-pipeline.md §a9-gpu-compute-compositor
#[test]
fn shader_validates_with_naga() {
    let module = module();
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&module)
    .expect("WGSL validates");
    let entries: Vec<(&str, [u32; 3])> = module
        .entry_points
        .iter()
        .map(|e| (e.name.as_str(), e.workgroup_size))
        .collect();
    assert_eq!(
        entries,
        [
            ("compose", [WORKGROUP, WORKGROUP, 1]),
            ("to_rgba", [WORKGROUP, WORKGROUP, 1])
        ]
    );
    const { assert!(WORKGROUP * WORKGROUP <= 256, "default invocation limit") };
    const { assert!(scene::BIN_SIZE.is_multiple_of(WORKGROUP)) };
}

/// The WGSL structs have exactly the byte layout `pack` writes.
// Covers: specs/client/render-pipeline.md §a9-gpu-compute-compositor
#[test]
fn shader_struct_layouts_match_packing() {
    let module = module();
    let mut layouter = naga::proc::Layouter::default();
    layouter.update(module.to_ctx()).expect("layout");
    let layout = |name: &str| {
        let (handle, ty) = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some(name))
            .expect("struct exists");
        let naga::TypeInner::Struct { members, span } = &ty.inner else {
            panic!("{name} is a struct");
        };
        assert_eq!(layouter[handle].size, *span);
        let offsets: Vec<(String, u32)> = members
            .iter()
            .map(|m| (m.name.clone().unwrap_or_default(), m.offset))
            .collect();
        (*span, offsets)
    };
    let owned = |v: &[(&str, u32)]| -> Vec<(String, u32)> {
        v.iter().map(|&(n, o)| (n.to_string(), o)).collect()
    };
    assert_eq!(
        layout("Item"),
        (
            ITEM_SIZE as u32,
            owned(&[("area", 0), ("texel", 16), ("shade", 32), ("blend", 48)])
        )
    );
    assert_eq!(
        layout("Params"),
        (
            PARAMS_SIZE as u32,
            owned(&[
                ("size", 0),
                ("bins", 8),
                ("item_count", 16),
                ("map_rows", 20),
                ("pages", 24),
                ("pad", 28)
            ])
        )
    );
}

/// Bindings in the shader are the ones `device.rs` binds and `pack.rs`
/// documents.
// Covers: specs/client/render-pipeline.md §a9-gpu-compute-compositor
#[test]
fn shader_bindings() {
    let module = module();
    let mut globals: Vec<(u32, u32, String)> = module
        .global_variables
        .iter()
        .filter_map(|(_, g)| {
            let b = g.binding.as_ref()?;
            Some((b.group, b.binding, g.name.clone().unwrap_or_default()))
        })
        .collect();
    globals.sort();
    let names: Vec<(u32, u32, &str)> = globals
        .iter()
        .map(|(g, b, n)| (*g, *b, n.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            (0, 0, "params"),
            (0, 1, "items"),
            (0, 2, "bin_ranges"),
            (0, 3, "bin_items"),
            (0, 4, "maps"),
            (0, 5, "atlas"),
            (0, 6, "indices"),
            (0, 7, "palette"),
            (0, 8, "rgba"),
        ]
    );
}

// Covers: specs/client/render-pipeline.md §a9-gpu-compute-compositor
#[test]
fn item_and_params_bytes_are_little_endian() {
    let item = GpuItem {
        area: [1, 2, 3, 0x0102_0304],
        texel: [5, 6],
        page: 7,
        shade_len: 2,
        shade: [8, 9, 0, 0],
        blend: 1,
        blend_base: 0xAABB_CCDD,
    };
    let b = item.to_le_bytes();
    assert_eq!(&b[0..4], &[1, 0, 0, 0]);
    assert_eq!(&b[12..16], &[4, 3, 2, 1]);
    assert_eq!(
        &b[16..32],
        &[5, 0, 0, 0, 6, 0, 0, 0, 7, 0, 0, 0, 2, 0, 0, 0]
    );
    assert_eq!(&b[32..36], &[8, 0, 0, 0]);
    assert_eq!(&b[48..56], &[1, 0, 0, 0, 0xDD, 0xCC, 0xBB, 0xAA]);
    assert_eq!(&b[56..64], &[0; 8]);
    assert_eq!(GpuItem::from_le_bytes(&b), item);

    let params = Params {
        width: 800,
        height: 600,
        cols: 25,
        rows: 19,
        item_count: 3,
        map_rows: 513,
        pages: 2,
    };
    let b = params.to_le_bytes();
    assert_eq!(
        b,
        [
            0x20, 3, 0, 0, 0x58, 2, 0, 0, 25, 0, 0, 0, 19, 0, 0, 0, 3, 0, 0, 0, 1, 2, 0, 0, 2, 0,
            0, 0, 0, 0, 0, 0
        ]
    );
    assert_eq!(Params::from_le_bytes(&b), params);
}

/// Exact packed buffers of a small list: areas in view coordinates, atlas
/// texels of the area's corner, bin prefix sums, map bytes unchanged.
// Covers: specs/client/render-pipeline.md §a9-gpu-compute-compositor
#[test]
fn pack_vector() {
    let frames = vec![FrameImage {
        width: 40,
        height: 3,
        pixels: vec![1; 120],
    }];
    let mut maps = MapTable::new();
    let m = maps.push(harness::map_with(&[(1, 2)]));
    let view = Rect::new(-10, 0, 64, 40);
    let mut a = DrawItem::new(scene::FrameId(0), -20, 30);
    a.shade = scene::ShadeChain::new(&[m]).unwrap();
    a.clip = Rect::new(-100, -100, 1000, 1000);
    let b = DrawItem::new(scene::FrameId(0), 500, 0); // off the view
    let items = vec![a, b];
    let slots = vec![AtlasSlot {
        page: 0,
        x: 100,
        y: 200,
        w: 40,
        h: 3,
    }];
    let bins = scene::bin(&items, &frames, &maps, view).unwrap();
    let p = pack(&items, &bins, &frames, &slots, &maps, 1).unwrap();
    // Item a: image x −20..20, clip wider, view x −10..54 → area x 0..30, y 30..33;
    // texel x = 100 + (−10 − −20) = 110.
    assert_eq!(
        p.items[0],
        GpuItem {
            area: [0, 30, 30, 33],
            texel: [110, 200],
            page: 0,
            shade_len: 1,
            shade: [0, 0, 0, 0],
            blend: pack::BLEND_OPAQUE,
            blend_base: 0,
        }
    );
    assert_eq!(p.items[1].area, [0; 4]);
    // 2 × 2 bins; a touches bin (0, 0) (y 30..32) and (0, 1) (y 32..33).
    assert_eq!(p.bin_ranges, [0, 1, 1, 2, 2]);
    assert_eq!(p.bin_items, [0, 0]);
    assert_eq!(p.maps_bytes(), maps.rows()[0].to_vec());
    assert_eq!(p.params.map_rows, 1);
    assert_eq!(p.items_bytes().len(), 2 * ITEM_SIZE);
    assert_eq!(p.bin_ranges_bytes().len(), 5 * 4);
}

/// Storage buffers are never empty.
// Covers: specs/client/render-pipeline.md §a9-gpu-compute-compositor
#[test]
fn empty_list_pads_buffers() {
    let case = case_named("empty");
    let (atlas, p) = prepare(&case).unwrap();
    assert_eq!(atlas.page_count(), 0);
    assert_eq!(p.items_bytes(), vec![0; ITEM_SIZE]);
    assert_eq!(p.bin_items_bytes(), vec![0; 4]);
    assert_eq!(p.maps_bytes(), vec![0; 256]);
    assert_eq!(p.bin_ranges, vec![0; 25 * 19 + 1]);
    assert_eq!(emulate(&p, atlas.pages()).unwrap(), vec![0; 800 * 600]);
}

/// The shader's algorithm on the packed bytes equals the CPU reference
/// (and the binned CPU model) on every synthetic case.
// Covers: specs/client/render-pipeline.md §a9-gpu-compute-compositor, §a8-cpu-reference-compositor
#[test]
fn emulated_shader_matches_cpu_on_all_cases() {
    let all = cases();
    assert_eq!(all.len(), 12);
    for case in &all {
        let reference = cpu(case);
        let bins = scene::bin(&case.items, &case.frames, &case.maps, case.view).unwrap();
        let binned =
            scene::compose_binned(&case.items, &bins, &case.frames, &case.maps, case.view).unwrap();
        assert_eq!(binned, reference, "{}", case.name);
        let (atlas, packed) = prepare(case).unwrap();
        let out = emulate(&packed, atlas.pages()).unwrap();
        let d = diff(&reference, &out, case.view.width, 1);
        assert_eq!(d.differing, 0, "{}: {d:?}", case.name);
    }
}

/// The cases exercise what they claim: something drawn, two pages, every
/// blend op, chains up to 4, off-screen and empty items.
#[test]
fn cases_are_not_trivial() {
    for case in cases() {
        let drawn = cpu(&case).iter().filter(|&&i| i != 0).count();
        assert_eq!(drawn == 0, case.name == "empty", "{}", case.name);
    }
    assert_eq!(prepare(&case_named("two-pages")).unwrap().0.page_count(), 2);
    let stress = case_named("stress");
    let (_, p) = prepare(&stress).unwrap();
    assert!(p.items.iter().any(|i| i.blend == pack::BLEND_INDEX_TABLE));
    assert!(p.items.iter().any(|i| i.shade_len == 4));
    assert!(p.items.iter().any(|i| i.area == [0; 4]));
    assert!(stress.items.iter().any(|i| i.frame == scene::FrameId(0)));
}

/// M08: the comparison reports exactly the bytes changed, in the CPU
/// reference (what `--perturb N` does) or in the packed GPU input.
// Covers: specs/client/render-pipeline.md §a10-verify-harness-extension
#[test]
fn perturbations_are_reported_exactly() {
    let case = case_named("stress");
    let reference = cpu(&case);
    let (atlas, mut packed) = prepare(&case).unwrap();
    let out = emulate(&packed, atlas.pages()).unwrap();
    for n in [1, 7, 1000] {
        let mut bad = reference.clone();
        assert_eq!(perturb(&mut bad, n), n);
        let d = diff(&bad, &out, case.view.width, 1);
        assert_eq!(d.differing, n as u64);
        assert_eq!(d.first.map(|f| (f.0, f.1)), Some((0, 0)));
        let rgba = diff(
            &scene::to_rgba(&bad, &case.palette),
            &scene::to_rgba(&out, &case.palette),
            case.view.width,
            4,
        );
        assert_eq!(rgba.differing, n as u64);
    }
    // A wrong shade row in the packed table must show in the image.
    let row = packed.items.iter().find(|i| i.shade_len > 0).unwrap().shade[0] as usize;
    for b in &mut packed.maps[row * 256..(row + 1) * 256] {
        *b = b.wrapping_add(1);
    }
    let broken = emulate(&packed, atlas.pages()).unwrap();
    assert!(diff(&reference, &broken, case.view.width, 1).differing > 0);
}

// Covers: specs/client/render-pipeline.md §a10-verify-harness-extension
#[test]
fn diff_counts_length_mismatch_and_first() {
    let d = diff(&[1, 2, 3, 4], &[1, 9, 3], 2, 1);
    assert_eq!(d.differing, 2);
    assert_eq!(d.first, Some((1, 0, 2, 9)));
    assert_eq!(perturb(&mut [0u8; 3], 10), 3);
    assert_eq!(perturb(&mut [], 1), 0);
}

/// Strict inputs (M07): everything the CPU rejects, `pack` rejects, plus
/// missing, mis-sized and out-of-atlas slots and foreign bins.
// Covers: specs/client/render-pipeline.md §a9-gpu-compute-compositor, §edge-cases-original-bugs
#[test]
fn pack_rejects_bad_input() {
    let case = case_named("vector-key-order");
    let atlas = AtlasFrames::from_images(&case.frames, 1).unwrap();
    let bins = scene::bin(&case.items, &case.frames, &case.maps, case.view).unwrap();
    let ok = |slots: &Vec<AtlasSlot>, pages| {
        pack(&case.items, &bins, &case.frames, slots, &case.maps, pages)
    };
    assert!(ok(&atlas.slots, 1).is_ok());
    assert!(matches!(
        ok(&atlas.slots[..1].to_vec(), 1),
        Err(GpuError::SlotMissing { .. })
    ));
    let mut wrong = atlas.slots.clone();
    wrong[1].w += 1;
    assert!(matches!(ok(&wrong, 1), Err(GpuError::SlotSize { .. })));
    assert!(matches!(
        ok(&atlas.slots, 0),
        Err(GpuError::SlotPage { .. })
    ));
    let mut edge = atlas.slots.clone();
    edge[0].x = crate::frames::PAGE_SIZE - 2;
    assert!(matches!(ok(&edge, 1), Err(GpuError::SlotPage { .. })));

    // Bins carry their view: bins of another view pack that view.
    let other = scene::bin(&case.items, &case.frames, &case.maps, Rect::FRAME).unwrap();
    let p = pack(
        &case.items,
        &other,
        &case.frames,
        &atlas.slots,
        &case.maps,
        1,
    )
    .unwrap();
    assert_eq!(
        (p.view, p.params.cols, p.params.rows),
        (Rect::FRAME, 25, 19)
    );
    // Bins of another list of the same length.
    let mut moved = case.items.clone();
    moved[0].x = 100; // off the view: no longer in bin 0
    assert!(matches!(
        pack(&moved, &bins, &case.frames, &atlas.slots, &case.maps, 1),
        Err(GpuError::BinsMismatch)
    ));
    // An item the CPU rejects.
    let mut flipped = case.items.clone();
    flipped[0].flip_x = true;
    assert!(matches!(
        pack(&flipped, &bins, &case.frames, &atlas.slots, &case.maps, 1),
        Err(GpuError::Scene(scene::SceneError::Item { index: 0, .. }))
    ));
    // Pages given at dispatch must be the packed count.
    let p = ok(&atlas.slots, 1).unwrap();
    assert!(matches!(
        emulate(&p, &[]),
        Err(GpuError::PageCount {
            pages: 0,
            packed: 1
        })
    ));
}

fn gpu() -> Gpu {
    let (gpu, info) = Gpu::headless().expect("a GPU adapter");
    println!(
        "adapter: {} ({:?}, {:?})",
        info.name, info.backend, info.device_type
    );
    gpu
}

/// Local, needs a GPU: every case byte-identical on the GPU.
/// `cargo test -p d2-client --lib gpu_compositor::tests::gpu_matches_cpu -- --ignored --nocapture`
// Covers: specs/client/render-pipeline.md §a9-gpu-compute-compositor
#[test]
#[ignore = "needs a GPU"]
fn gpu_matches_cpu() {
    let gpu = gpu();
    for case in cases() {
        let report = harness::compare(&gpu, &case, 0).unwrap();
        println!("{report}");
        assert!(report.is_match(), "{report}");
    }
}

/// Local, needs a GPU (M08): with 7 reference bytes changed, every case
/// reports exactly 7 differing bytes and 7 differing pixels.
// Covers: specs/client/render-pipeline.md §a10-verify-harness-extension
#[test]
#[ignore = "needs a GPU"]
fn gpu_perturb_reports_exactly_n() {
    let gpu = gpu();
    for case in cases() {
        let report = harness::compare(&gpu, &case, 7).unwrap();
        println!("{report}");
        assert_eq!(report.indices.differing, 7, "{report}");
        assert_eq!(report.rgba.differing, 7, "{report}");
    }
}
