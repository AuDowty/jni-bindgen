# jni-bindgen

Generate Rust [`jni`](https://crates.io/crates/jni)-crate wrappers from compiled Java `.class` files.

## Install

```
cargo install --git https://github.com/AuDowty/jni-bindgen
```

## Use

```
jni-bindgen Foo.class                  # print to stdout
jni-bindgen Foo.class -o foo.rs        # write to file
jni-bindgen Foo.class --name MyFoo     # override struct name
```

Given a class like:

```java
public class Hello {
    public Hello() {}
    public String greet(String name) { return "hi " + name; }
    public int add(int a, int b) { return a + b; }
    public static String version() { return "1.0"; }
}
```

`jni-bindgen Hello.class` emits a `Hello<'local>` struct with `new()`, `greet()`, `add()`, and `version()` that call through `JNIEnv` with the correct descriptors baked in.

Handles constructors, instance methods, static methods. Primitives, `String`, and opaque object types are wrapped; arrays and inner classes pass through as `JObject`.

## License

MIT
