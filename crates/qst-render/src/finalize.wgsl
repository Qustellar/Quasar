struct IndirectArgs {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
};

@group(0) @binding(0) var<storage, read> visible_count: array<u32>;
@group(0) @binding(1) var<storage, read_write> indirect: IndirectArgs;
struct Params { count: u32, index_count: u32, _pad0: u32, _pad1: u32 };
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(1)
fn main() {
    indirect.index_count = params.index_count;
    indirect.instance_count = visible_count[0];
    indirect.first_index = 0u;
    indirect.base_vertex = 0;
    indirect.first_instance = 0u;
}
