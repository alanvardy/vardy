# Task — Check for CSS drift on CI

The compiled CSS artifact `static/site.css` is committed to git and rebuilt
from source (`css/site.css`) by the Tailwind build script. A local script
checks that the committed artifact matches fresh build output, but no GitHub
workflow runs this check. Add the CSS drift check to CI so stale committed
CSS is caught on pull requests and pushes.