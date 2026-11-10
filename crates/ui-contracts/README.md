# NeoCogi UI contracts

Canonical UI, node, document, GFX and Skin declarations, typed caller conveniences and shared bounded values.
This crate contains no retained runtime, concrete widget implementation, image decoder or font rasterizer.
Enable `build-support` in build dependencies for consumer-owned NCIDL generation using packaged schemas.
Native and browser implementations live in `neocogi-ui`; `neocogi-ui-dylib` owns only the final native export.
