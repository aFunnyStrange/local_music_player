# Third-party components / 第三方组件

The MIT license covers Shiyin's original code and documentation only. Bundled components
retain their own licenses. See `licenses/` in the binary package and
`_internal/vlc/COPYING.txt` for the included license texts.

MIT 仅覆盖拾音的原创代码与文档。运行库仍适用各自的许可证；二进制包保留了相关许可文本。

| Component | Version | Upstream / source |
| --- | --- | --- |
| CPython | build environment (3.12.10 in CI) | https://www.python.org/downloads/release/python-31210/ |
| Tcl/Tk | bundled with CPython | https://www.tcl.tk/software/tcltk/ |
| Mutagen | 1.48.1 | https://github.com/quodlibet/mutagen |
| python-vlc | 3.0.21203 | https://github.com/oaubert/python-vlc |
| pywin32 | 312 | https://github.com/mhammond/pywin32 |
| VLC / libVLC and plugins | 3.0.21 | https://download.videolan.org/pub/videolan/vlc/3.0.21/ |
| PyInstaller bootloader | 6.22.0 | https://github.com/pyinstaller/pyinstaller |

VLC is shipped as the unmodified upstream Windows directory; the player loads libVLC
in process and does not launch the VLC executable. Upstream VLC source is available at
https://download.videolan.org/pub/videolan/vlc/3.0.21/vlc-3.0.21.tar.xz;
build scripts and dependency recipes are in that source tree. Distributors must fulfill
the corresponding source and notice obligations of shipped components; an MIT project
license does not replace those obligations. The release workflow creates a **draft** so
the maintainer can attach the required corresponding sources and review distribution
before publication. A list of upstream links alone is not a complete source-delivery process.

发布者应按实际包含组件的许可证提供对应源码与声明，发布流程默认生成草稿，
由维护者补齐对应源码材料并检查后再公开。仅列出上游链接不等同于完成源码提供。

No personal music, library settings or credentials are included in the release bundle.
Music files are outside this project's license grant.
