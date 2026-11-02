((hlsl_block
  (code) @injection.content)
  (#set! injection.language "hlsl"))

((cg_block
  (code) @injection.content)
  (#set! injection.language "hlsl"))

((glsl_block
  (code) @injection.content)
  (#set! injection.language "glsl"))
