/// VERTEX SHADER
/// =============

@group(0) @binding(0)
var<uniform> u_projection: mat4x4<f32>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
};

fn get_rectangle_vertex_coordinates(vertex_index: u32) -> vec2<f32> {
    switch (vertex_index) {
        case 0u: {
            return vec2<f32>(-0.5, 0.5);   // top left
        }
        case 1u: {
	        return vec2<f32>(0.5, 0.5);    // top right
        }
        case 2u: {
	        return vec2<f32>(-0.5, -0.5);  // bottom left
        }
        case 3u: {
	        return vec2<f32>(-0.5, -0.5);  // bottom left
        }
        case 4u: {
	        return vec2<f32>(0.5, -0.5);   // bottom right
        }
        case 5u: {
	        return vec2<f32>(0.5, 0.5);    // top right
        }
        default: {
            return vec2<f32>(0.0, 0.0);    // unreachable
        }
    }
}

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    let coords = get_rectangle_vertex_coordinates(in_vertex_index);

    var out: VertexOutput;
    out.position = u_projection *  vec4<f32>(coords.x*100+200, coords.y*100+200, 0.0, 1.0);
    return out;
}

/// FRAGMENT SHADER
/// ===============

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.3, 0.2, 0.1, 1.0);
}