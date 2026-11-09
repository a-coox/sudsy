# Fenced code

Config as JSON:

```json
{"name":"app","version":"1.0.0","scripts":{"build":"tsc","test":"jest"},"files":["dist","README.md"]}
```

- A list item with YAML:

  ```yaml
  services:
      web:
          image:   nginx:1.27
          ports: ["80:80", "443:443"]
  ```

> A quote with shell:
>
> ```sh
> set -e
> cd build && cmake .. && make -j4
> ```

A Dockerfile:

```dockerfile
FROM alpine:3.20
RUN apk add --no-cache curl && \
    curl -fsSL https://example.com/install.sh | sh
CMD ["app"]
```

Code of a language nobody here formats:

```rust
fn main() { println!("hi"); }
```
