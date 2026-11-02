(comment) @comment

(string) @string

(escape_sequence) @string.escape

(number) @number

[
  (true)
  (false)
] @boolean

(null) @constant.builtin

[
  ","
  ":"
] @punctuation.delimiter

[
  "{"
  "}"
  "["
  "]"
] @punctuation.bracket

(pair
  key: (string) @property)

(pair
  key: (string (string_content) @_key)
  (#any-of? @_key "name" "rootNamespace" "reference")
  value: (string (string_content) @type))

((string (string_content) @string.special)
  (#match? @string.special "^GUID:"))

(pair
  key: (string (string_content) @_key)
  (#any-of? @_key "includePlatforms" "excludePlatforms")
  value: (array (string (string_content) @constant.builtin)))

(pair
  key: (string (string_content) @_key)
  (#eq? @_key "defineConstraints")
  value: (array (string (string_content) @constant)))

(pair
  key: (string (string_content) @_key)
  (#eq? @_key "precompiledReferences")
  value: (array (string (string_content) @string)))

(pair
  key: (string (string_content) @_key)
  (#eq? @_key "versionDefines")
  value: (array
    (object
      (pair
        key: (string (string_content) @_vk)
        (#eq? @_vk "name")
        value: (string (string_content) @string))
      (pair
        key: (string (string_content) @_vk2)
        (#eq? @_vk2 "expression")
        value: (string (string_content) @string.special))
      (pair
        key: (string (string_content) @_vk3)
        (#eq? @_vk3 "define")
        value: (string (string_content) @constant)))))
