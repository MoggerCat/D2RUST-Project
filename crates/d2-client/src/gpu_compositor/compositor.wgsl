// Spec: specs/client/render-pipeline.md (A9)
// GPU compute compositor. Same formulas as the CPU reference
// (`scene::cpu::compose_binned`): per pixel, walk the pixel's bin list in
// order; index 0 of the frame leaves the pixel unchanged; otherwise the
// shade chain maps the index (A4) and the blend op combines it with the
// pixel's value (A5). Integers only; frames are read with textureLoad.
// Buffer layouts: `pack.rs` (all little-endian).

struct Params {
    size: vec2<u32>,   // view width, height
    bins: vec2<u32>,   // bin columns, rows
    item_count: u32,
    map_rows: u32,
    pages: u32,
    pad: u32,
}

struct Item {
    area: vec4<u32>,   // x0, y0, x1, y1 in view pixels, ends exclusive
    texel: vec4<u32>,  // atlas x, y of the area's top-left; page; shade length
    shade: vec4<u32>,  // map rows, used in order
    blend: vec4<u32>,  // op (0 opaque, 1 index table), table base row, 0, 0
}

const BIN_SIZE: u32 = 32u;
const BLEND_OPAQUE: u32 = 0u;

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> items: array<Item>;
@group(0) @binding(2) var<storage, read> bin_ranges: array<u32>;
@group(0) @binding(3) var<storage, read> bin_items: array<u32>;
@group(0) @binding(4) var<storage, read> maps: array<u32>;
@group(0) @binding(5) var atlas: texture_2d_array<u32>;
@group(0) @binding(6) var<storage, read_write> indices: array<u32>;
@group(0) @binding(7) var<storage, read> palette: array<u32, 256>;
@group(0) @binding(8) var<storage, read_write> rgba: array<u32>;

// Byte `b` of map row `row`: rows are 256 bytes, four per word, little-endian.
fn map_byte(row: u32, b: u32) -> u32 {
    return (maps[(row << 6u) + (b >> 2u)] >> ((b & 3u) << 3u)) & 0xffu;
}

fn shade(it: Item, index: u32) -> u32 {
    var s = index;
    for (var j = 0u; j < it.texel.w; j++) {
        s = map_byte(it.shade[j], s);
    }
    return s;
}

@compute @workgroup_size(16, 16, 1)
fn compose(@builtin(global_invocation_id) gid: vec3<u32>) {
    let px = gid.x;
    let py = gid.y;
    if px >= params.size.x || py >= params.size.y {
        return;
    }
    let bin = (py / BIN_SIZE) * params.bins.x + px / BIN_SIZE;
    var value = 0u;
    for (var k = bin_ranges[bin]; k < bin_ranges[bin + 1u]; k++) {
        let it = items[bin_items[k]];
        if px < it.area.x || px >= it.area.z || py < it.area.y || py >= it.area.w {
            continue;
        }
        let t = vec2<u32>(it.texel.x + (px - it.area.x), it.texel.y + (py - it.area.y));
        let src = textureLoad(atlas, t, it.texel.z, 0).r;
        if src == 0u {
            continue;
        }
        let s = shade(it, src);
        if it.blend.x == BLEND_OPAQUE {
            value = s;
        } else {
            value = map_byte(it.blend.y + s, value);
        }
    }
    indices[py * params.size.x + px] = value;
}

// Index framebuffer -> RGBA8 bytes through the frame palette: a bit copy of
// the palette word, so no color conversion can change a byte.
@compute @workgroup_size(16, 16, 1)
fn to_rgba(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.size.x || gid.y >= params.size.y {
        return;
    }
    let i = gid.y * params.size.x + gid.x;
    rgba[i] = palette[indices[i] & 0xffu];
}
