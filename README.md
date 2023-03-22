Uses skia ran with a rust bridge compiled into webassembly, for using chrome internal gpu-painting mechanisms without the Blink engines overhead. 

Instead of parsing into DOM/CSSOM, building layout object tree etc, you can just instantly use chromes internal document-painting tools to raster whatever document-like thing you want on your screen


Then build the example:

```shell
make build
```

Start a web server (requires Python 3):

```shell
make serve
```

