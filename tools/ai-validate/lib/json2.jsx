// lib/json2.jsx — minimal JSON polyfill for ExtendScript.
// Only provides JSON.stringify and JSON.parse if the host engine lacks them.
// Adapted from Douglas Crockford's json2.js (public domain).

if (typeof JSON === "undefined") {
    JSON = {};
}

(function () {
    "use strict";

    var rx_escapable = /[\\\"\x00-\x1f\x7f-\x9f]/g;
    var meta = {
        "\b": "\\b",
        "\t": "\\t",
        "\n": "\\n",
        "\f": "\\f",
        "\r": "\\r",
        "\"": "\\\"",
        "\\": "\\\\"
    };

    function quote(string) {
        rx_escapable.lastIndex = 0;
        return rx_escapable.test(string)
            ? "\"" + string.replace(rx_escapable, function (a) {
                var c = meta[a];
                return typeof c === "string"
                    ? c
                    : "\\u" + ("0000" + a.charCodeAt(0).toString(16)).slice(-4);
            }) + "\""
            : "\"" + string + "\"";
    }

    function str(key, holder, indent, gap) {
        var value = holder[key];
        var i, partial, v;

        if (value === null) {
            return "null";
        }
        if (value === undefined) {
            return undefined;
        }

        var type = typeof value;

        if (type === "boolean") {
            return String(value);
        }
        if (type === "number") {
            return isFinite(value) ? String(value) : "null";
        }
        if (type === "string") {
            return quote(value);
        }

        // Array
        if (value instanceof Array) {
            partial = [];
            var childIndent = indent + gap;
            for (i = 0; i < value.length; i++) {
                var item = str(i, value, childIndent, gap);
                partial.push(item === undefined ? "null" : item);
            }
            if (partial.length === 0) {
                return "[]";
            }
            if (gap) {
                return "[\n" + childIndent + partial.join(",\n" + childIndent) + "\n" + indent + "]";
            }
            return "[" + partial.join(",") + "]";
        }

        // Object
        if (type === "object") {
            partial = [];
            var childIndent2 = indent + gap;
            for (var k in value) {
                if (value.hasOwnProperty(k)) {
                    v = str(k, value, childIndent2, gap);
                    if (v !== undefined) {
                        partial.push(quote(k) + (gap ? ": " : ":") + v);
                    }
                }
            }
            if (partial.length === 0) {
                return "{}";
            }
            if (gap) {
                return "{\n" + childIndent2 + partial.join(",\n" + childIndent2) + "\n" + indent + "}";
            }
            return "{" + partial.join(",") + "}";
        }

        return undefined;
    }

    if (typeof JSON.stringify !== "function") {
        JSON.stringify = function (value, replacer, space) {
            var gap = "";
            if (typeof space === "number") {
                for (var i = 0; i < space; i++) {
                    gap += " ";
                }
            } else if (typeof space === "string") {
                gap = space;
            }
            return str("", {"": value}, "", gap);
        };
    }

    if (typeof JSON.parse !== "function") {
        JSON.parse = function (text) {
            // Minimal — only used for reading simple config files.
            // Not safe for untrusted input, but fine for our own reports.
            return eval("(" + text + ")");
        };
    }
})();
