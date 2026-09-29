// Syntax highlighting for ```volt blocks. mdBook's bundled highlight.js
// does not know Volt; register it and highlight the Volt blocks again.
// The keyword list follows crates/volt-syntax/src/token.rs.
(function () {
    "use strict";
    if (typeof hljs === "undefined") {
        return;
    }
    hljs.registerLanguage("volt", function (hljs) {
        return {
            name: "Volt",
            keywords: {
                keyword:
                    "module in out inout reg let wire on comb domain if else match for as fn " +
                    "const type struct enum extern package use pub requires ensures invariant " +
                    "cover assert assume posedge negedge pipeline stage stall flush test",
                type: "bool clock reset bits",
                literal: "true false none active_high active_low sync async",
                built_in:
                    "step assert_eq assert_true assert_false prev read_hex load " +
                    "sync3 declassify",
            },
            contains: [
                hljs.C_LINE_COMMENT_MODE,
                hljs.C_BLOCK_COMMENT_MODE,
                hljs.QUOTE_STRING_MODE,
                { className: "type", begin: /\b[ui]\d+\b/ },
                { className: "meta", begin: /@[A-Za-z_]\w*/ },
                {
                    className: "number",
                    begin: /\b(0x[0-9A-Fa-f_]+|0b[01_]+|\d[\d_]*)\b/,
                    relevance: 0,
                },
            ],
        };
    });
    document.querySelectorAll("code.language-volt").forEach(function (block) {
        hljs.highlightBlock(block);
    });
})();
