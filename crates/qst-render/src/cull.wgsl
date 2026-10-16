struct Globals {
    view_projection: mat4x4<f32>,
    light_direction: vec4<f32>,
    light_color: vec4<f32>,
    ambient: vec4<f32>,
};

struct CullInstance {
    model: mat4x4<f32>,
    color: vec4<f32>,
    bounds: vec4<f32>,
    skin_base: u32,
    skin_count: u32,
    _padding_0: u32,
    _padding_1: u32,
};

struct Instance {
    model: mat4x4<f32>,
    color: vec4<f32>,
    skin_base: u32,
    skin_count: u32,
    _padding_0: u32,
    _padding_1: u32,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var<storage, read> input_instances: array<CullInstance>;
@group(0) @binding(2) var<storage, read_write> output_instances: array<Instance>;
@group(0) @binding(3) var<storage, read_write> visible_count: atomic<u32>;

struct Params { count: u32, index_count: u32, _pad0: u32, _pad1: u32 };
@group(0) @binding(4) var<uniform> params: Params;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.count) {
        return;
    }
    let instance = input_instances[id.x];
    let center = instance.model * vec4<f32>(instance.bounds.xyz, 1.0);
    let scale = max(length(instance.model[0].xyz), max(length(instance.model[1].xyz), length(instance.model[2].xyz)));
    let radius = abs(instance.bounds.w) * scale;
    let rows = transpose(globals.view_projection);
    let planes = array<vec4<f32>, 6>(rows[3] + rows[0], rows[3] - rows[0],
        rows[3] + rows[1], rows[3] - rows[1], rows[2], rows[3] - rows[2]);
    var visible = true;
    for (var i = 0u; i < 6u; i += 1u) {
        if (dot(planes[i], center) < -radius * length(planes[i].xyz)) { visible = false; }
    }
    if (visible) {
        let index = atomicAdd(&visible_count, 1u);
        output_instances[index].model = instance.model;
        output_instances[index].color = instance.color;
        output_instances[index].skin_base = instance.skin_base;
        output_instances[index].skin_count = instance.skin_count;
        output_instances[index]._padding_0 = 0u;
        output_instances[index]._padding_1 = 0u;
    }
}
