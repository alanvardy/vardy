# Task: Make used links same colour as unused links

Visited ("used") links currently render in the browser-default purple, which reads
poorly against the site's dark theme. The goal is to make visited links display the
same colour as unvisited links across every page of the site, without regressing the
existing hover/active link styles. This is a frontend/styling change that needs to
flow correctly through the Tailwind v4 source → generated CSS pipeline and the static
asset cache-busting mechanism.