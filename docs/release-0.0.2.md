# Quasar 0.0.2

`0.0.2` is the Game Development Foundation preview. It keeps the 0.0.1 facade and adds:

- schema 1/2 to schema 3 scene migration with stable IDs and local transform data;
- fixed runtime stage registration and keyboard/mouse input snapshots;
- RGBA8 PNG/JPEG texture assets, sampler metadata, and glTF UV/tangent/material dependencies;
- transform animation clips with linear/step sampling and playback state;
- audio clip/player state APIs with rodio and null backend boundaries;
- scene create/delete/duplicate/reparent helpers and a repository acceptance fixture;
- expanded frame, cache, import, physics, animation, and render diagnostics.

Deferred to 0.0.3 are skinning, compressed textures, deferred rendering, GPU culling, spatial audio, scripting, networking, and a standalone editor binary.
