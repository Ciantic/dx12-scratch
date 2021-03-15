# DirectX12 Scratch

This is just my attempt to recreate [triangle example of DirectX Graphics Samples](https://github.com/microsoft/DirectX-Graphics-Samples/tree/master/Samples/Desktop/D3D12HelloWorld/src/HelloTriangle) in Rust.

Currently only functional is the [`WinAPI Rust`](./winapi-rs/) implementation that uses winapi bindings.

`Windows-Rs` implementation does not work yet, because it doesn't support all the structures yet. E.g. [D3D12_BLEND_DESC](https://microsoft.github.io/windows-docs-rs/doc/bindings/windows/win32/direct3d12/struct.D3D12_BLEND_DESC.html) says `NOT_YET_SUPPORTED_TYPE` for the render_taget as of today.

I might make the example in other languages too.