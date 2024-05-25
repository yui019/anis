/// VERTEX SHADER
/// =============

struct RectangleDrawData {
    pos: vec2<f32>,
    size: vec2<f32>,
    color: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> u_projection: mat4x4<f32>;

@group(0) @binding(1)
var<storage, read> s_rectangles: array<RectangleDrawData>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
};

fn choose_vertex_corner(
    vertex_index: u32, tl: vec2<f32>, tr: vec2<f32>, bl: vec2<f32>, br: vec2<f32>
) -> vec2<f32> {
    switch (vertex_index) {
        case 0u: {
            return tl;
        }
        case 1u: {
	        return tr;
        }
        case 2u: {
	        return bl;
        }
        case 3u: {
	        return bl;
        }
        case 4u: {
	        return br;
        }
        case 5u: {
	        return tr;
        }
        default: {
            return tl; // unreachable
        }
    }
}

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    let rectangle = s_rectangles[in_vertex_index / 6];

    let x = rectangle.pos.x;
    let y = rectangle.pos.y;
    let w = rectangle.size.x;
    let h = rectangle.size.y;

    let tl = vec2<f32>(x, y);
	let tr = vec2<f32>(x + w, y);
	let bl = vec2<f32>(x, y + h);
	let br = vec2<f32>(x + w, y + h);
    
    let coords = choose_vertex_corner(in_vertex_index % 6, tl, tr, bl, br);

    var out: VertexOutput;
    out.position = u_projection * vec4<f32>(
        coords.x,
        coords.y,
        0.0, 1.0
    );
    return out;
}

/// FRAGMENT SHADER
/// ===============

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.3, 0.2, 0.1, 1.0);
}