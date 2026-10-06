// Spec: specs/client/render-pipeline.md (A9)
// Spec: specs/render/composition.md (§3 frame cycle, §5 one pixel write)
// Spec: specs/render/shading.md (§4 tile light gradients)
// Spec: specs/render/blend-modes.md (§2 table orientation per drawer)
// GPU compute compositor. Same formulas as the CPU reference
// (`scene::cpu::compose_binned_frame`): per pixel, start from the base
// (the previous frame; 0 in rows below `clear_rows`), walk the pixel's bin
// list in order; index 0 of the frame leaves the pixel unchanged;
// otherwise the shade chain maps the index (A4), then the item's light
// gradient map of the pixel if any (shading §4), and the blend op combines
// it with the pixel's value (A5; `IndexTable` reads row = destination,
// column = source, `IndexTableSrcRow` the transpose); `clear_after` sets
// the result to 0. Integers only; frames are
// read with textureLoad.
// Buffer layouts: `pack.rs` (all little-endian).

struct Params {
    size: vec2<u32>,   // view width, height
    bins: vec2<u32>,   // bin columns, rows
    item_count: u32,
    map_rows: u32,
    pages: u32,
    clear_rows: u32,   // view rows 0..clear_rows start at 0 (StartDraw clear)
    clear_after: u32,  // 1: every pixel 0 after drawing (ClearScreen)
    pad0: u32,
    pad1: u32,
    pad2: u32,
}

struct Item {
    area: vec4<u32>,   // x0, y0, x1, y1 in view pixels, ends exclusive
    texel: vec4<u32>,  // atlas x, y of the area's top-left; page; shade length
    shade: vec4<u32>,  // map rows, used in order
    blend: vec4<u32>,  // op (0 opaque, 1 index table, 2 index table src row), table base row, 0, 0
    light: vec4<u32>,  // gradient kind (0 none, 1 wall, 2 RLE floor), light map 0 row, corners c0..c3 (bytes), dx | dy << 16
}

const BIN_SIZE: u32 = 32u;
const BLEND_OPAQUE: u32 = 0u;
const BLEND_INDEX_TABLE_SRC_ROW: u32 = 2u;
const GRADIENT_WALL: u32 = 1u;

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> items: array<Item>;
@group(0) @binding(2) var<storage, read> bin_ranges: array<u32>;
@group(0) @binding(3) var<storage, read> bin_items: array<u32>;
@group(0) @binding(4) var<storage, read> maps: array<u32>;
@group(0) @binding(5) var atlas: texture_2d_array<u32>;
@group(0) @binding(6) var<storage, read_write> indices: array<u32>;
@group(0) @binding(7) var<storage, read> palette: array<u32, 256>;
@group(0) @binding(8) var<storage, read_write> rgba: array<u32>;
@group(0) @binding(9) var<storage, read> base: array<u32>;

// Byte `i` of the base framebuffer: four pixels per word, little-endian.
fn base_byte(i: u32) -> u32 {
    return (base[i >> 2u] >> ((i & 3u) << 3u)) & 0xffu;
}

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

fn corner(it: Item, i: u32) -> i32 {
    return i32((it.light.z >> (8u * i)) & 0xffu);
}

// Light map row of area pixel (ax, ay) of a gradient-lit item: block
// column x, row r; a_r, b_r the row's left and right levels; G[a][b][x].
fn gradient_row(it: Item, ax: u32, ay: u32) -> u32 {
    var scale = 16;
    var shift = 7u;
    if it.light.x == GRADIENT_WALL {
        scale = 32;
        shift = 8u;
    }
    let x = i32(ax + (it.light.w & 0xffffu));
    let r = i32(ay + (it.light.w >> 16u));
    let a = (scale * corner(it, 0u) + r * (corner(it, 3u) - corner(it, 0u))) >> shift;
    let b = (scale * corner(it, 1u) + r * (corner(it, 2u) - corner(it, 1u))) >> shift;
    return it.light.y + u32((32 * a + x * (b - a)) >> 5u);
}

@compute @workgroup_size(16, 16, 1)
fn compose(@builtin(global_invocation_id) gid: vec3<u32>) {
    let px = gid.x;
    let py = gid.y;
    if px >= params.size.x || py >= params.size.y {
        return;
    }
    let bin = (py / BIN_SIZE) * params.bins.x + px / BIN_SIZE;
    let at = py * params.size.x + px;
    var value = 0u;
    if py >= params.clear_rows {
        value = base_byte(at);
    }
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
        var s = shade(it, src);
        if it.light.x != 0u {
            s = map_byte(gradient_row(it, px - it.area.x, py - it.area.y), s);
        }
        if it.blend.x == BLEND_OPAQUE {
            value = s;
        } else if it.blend.x == BLEND_INDEX_TABLE_SRC_ROW {
            value = map_byte(it.blend.y + s, value);
        } else {
            value = map_byte(it.blend.y + value, s);
        }
    }
    if params.clear_after != 0u {
        value = 0u;
    }
    indices[at] = value;
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
