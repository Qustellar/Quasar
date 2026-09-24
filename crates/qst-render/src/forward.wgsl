struct Globals {
    view_projection: mat4x4<f32>,
    light_direction: vec4<f32>,
    light_color: vec4<f32>,
    ambient: vec4<f32>,
};

struct Object {
    model: mat4x4<f32>,
    color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var<uniform> object: Object;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = globals.view_projection * object.model * vec4<f32>(input.position, 1.0);
    output.normal = normalize((object.model * vec4<f32>(input.normal, 0.0)).xyz);
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let diffuse = max(dot(normalize(input.normal), -normalize(globals.light_direction.xyz)), 0.0);
    let lighting = globals.ambient.xyz + diffuse * globals.light_color.xyz * globals.light_color.w;
    return vec4<f32>(object.color.xyz * lighting, object.color.w);
}
