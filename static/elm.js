(function(scope){
'use strict';

function F(arity, fun, wrapper) {
  wrapper.a = arity;
  wrapper.f = fun;
  return wrapper;
}

function F2(fun) {
  return F(2, fun, function(a) { return function(b) { return fun(a,b); }; })
}
function F3(fun) {
  return F(3, fun, function(a) {
    return function(b) { return function(c) { return fun(a, b, c); }; };
  });
}
function F4(fun) {
  return F(4, fun, function(a) { return function(b) { return function(c) {
    return function(d) { return fun(a, b, c, d); }; }; };
  });
}
function F5(fun) {
  return F(5, fun, function(a) { return function(b) { return function(c) {
    return function(d) { return function(e) { return fun(a, b, c, d, e); }; }; }; };
  });
}
function F6(fun) {
  return F(6, fun, function(a) { return function(b) { return function(c) {
    return function(d) { return function(e) { return function(f) {
    return fun(a, b, c, d, e, f); }; }; }; }; };
  });
}
function F7(fun) {
  return F(7, fun, function(a) { return function(b) { return function(c) {
    return function(d) { return function(e) { return function(f) {
    return function(g) { return fun(a, b, c, d, e, f, g); }; }; }; }; }; };
  });
}
function F8(fun) {
  return F(8, fun, function(a) { return function(b) { return function(c) {
    return function(d) { return function(e) { return function(f) {
    return function(g) { return function(h) {
    return fun(a, b, c, d, e, f, g, h); }; }; }; }; }; }; };
  });
}
function F9(fun) {
  return F(9, fun, function(a) { return function(b) { return function(c) {
    return function(d) { return function(e) { return function(f) {
    return function(g) { return function(h) { return function(i) {
    return fun(a, b, c, d, e, f, g, h, i); }; }; }; }; }; }; }; };
  });
}

function A2(fun, a, b) {
  return fun.a === 2 ? fun.f(a, b) : fun(a)(b);
}
function A3(fun, a, b, c) {
  return fun.a === 3 ? fun.f(a, b, c) : fun(a)(b)(c);
}
function A4(fun, a, b, c, d) {
  return fun.a === 4 ? fun.f(a, b, c, d) : fun(a)(b)(c)(d);
}
function A5(fun, a, b, c, d, e) {
  return fun.a === 5 ? fun.f(a, b, c, d, e) : fun(a)(b)(c)(d)(e);
}
function A6(fun, a, b, c, d, e, f) {
  return fun.a === 6 ? fun.f(a, b, c, d, e, f) : fun(a)(b)(c)(d)(e)(f);
}
function A7(fun, a, b, c, d, e, f, g) {
  return fun.a === 7 ? fun.f(a, b, c, d, e, f, g) : fun(a)(b)(c)(d)(e)(f)(g);
}
function A8(fun, a, b, c, d, e, f, g, h) {
  return fun.a === 8 ? fun.f(a, b, c, d, e, f, g, h) : fun(a)(b)(c)(d)(e)(f)(g)(h);
}
function A9(fun, a, b, c, d, e, f, g, h, i) {
  return fun.a === 9 ? fun.f(a, b, c, d, e, f, g, h, i) : fun(a)(b)(c)(d)(e)(f)(g)(h)(i);
}




var _JsArray_empty = [];

function _JsArray_singleton(value)
{
    return [value];
}

function _JsArray_length(array)
{
    return array.length;
}

var _JsArray_initialize = F3(function(size, offset, func)
{
    var result = new Array(size);

    for (var i = 0; i < size; i++)
    {
        result[i] = func(offset + i);
    }

    return result;
});

var _JsArray_initializeFromList = F2(function (max, ls)
{
    var result = new Array(max);

    for (var i = 0; i < max && ls.b; i++)
    {
        result[i] = ls.a;
        ls = ls.b;
    }

    result.length = i;
    return _Utils_Tuple2(result, ls);
});

var _JsArray_unsafeGet = F2(function(index, array)
{
    return array[index];
});

var _JsArray_unsafeSet = F3(function(index, value, array)
{
    var length = array.length;
    var result = new Array(length);

    for (var i = 0; i < length; i++)
    {
        result[i] = array[i];
    }

    result[index] = value;
    return result;
});

var _JsArray_push = F2(function(value, array)
{
    var length = array.length;
    var result = new Array(length + 1);

    for (var i = 0; i < length; i++)
    {
        result[i] = array[i];
    }

    result[length] = value;
    return result;
});

var _JsArray_foldl = F3(function(func, acc, array)
{
    var length = array.length;

    for (var i = 0; i < length; i++)
    {
        acc = A2(func, array[i], acc);
    }

    return acc;
});

var _JsArray_foldr = F3(function(func, acc, array)
{
    for (var i = array.length - 1; i >= 0; i--)
    {
        acc = A2(func, array[i], acc);
    }

    return acc;
});

var _JsArray_map = F2(function(func, array)
{
    var length = array.length;
    var result = new Array(length);

    for (var i = 0; i < length; i++)
    {
        result[i] = func(array[i]);
    }

    return result;
});

var _JsArray_indexedMap = F3(function(func, offset, array)
{
    var length = array.length;
    var result = new Array(length);

    for (var i = 0; i < length; i++)
    {
        result[i] = A2(func, offset + i, array[i]);
    }

    return result;
});

var _JsArray_slice = F3(function(from, to, array)
{
    return array.slice(from, to);
});

var _JsArray_appendN = F3(function(n, dest, source)
{
    var destLen = dest.length;
    var itemsToCopy = n - destLen;

    if (itemsToCopy > source.length)
    {
        itemsToCopy = source.length;
    }

    var size = destLen + itemsToCopy;
    var result = new Array(size);

    for (var i = 0; i < destLen; i++)
    {
        result[i] = dest[i];
    }

    for (var i = 0; i < itemsToCopy; i++)
    {
        result[i + destLen] = source[i];
    }

    return result;
});



// LOG

var _Debug_log = F2(function(tag, value)
{
	return value;
});

var _Debug_log_UNUSED = F2(function(tag, value)
{
	console.log(tag + ': ' + _Debug_toString(value));
	return value;
});


// TODOS

function _Debug_todo(moduleName, region)
{
	return function(message) {
		_Debug_crash(8, moduleName, region, message);
	};
}

function _Debug_todoCase(moduleName, region, value)
{
	return function(message) {
		_Debug_crash(9, moduleName, region, value, message);
	};
}


// TO STRING

function _Debug_toString(value)
{
	return '<internals>';
}

function _Debug_toString_UNUSED(value)
{
	return _Debug_toAnsiString(false, value);
}

function _Debug_toAnsiString(ansi, value)
{
	if (typeof value === 'function')
	{
		return _Debug_internalColor(ansi, '<function>');
	}

	if (typeof value === 'boolean')
	{
		return _Debug_ctorColor(ansi, value ? 'True' : 'False');
	}

	if (typeof value === 'number')
	{
		return _Debug_numberColor(ansi, value + '');
	}

	if (value instanceof String)
	{
		return _Debug_charColor(ansi, "'" + _Debug_addSlashes(value, true) + "'");
	}

	if (typeof value === 'string')
	{
		return _Debug_stringColor(ansi, '"' + _Debug_addSlashes(value, false) + '"');
	}

	if (typeof value === 'object' && '$' in value)
	{
		var tag = value.$;

		if (typeof tag === 'number')
		{
			return _Debug_internalColor(ansi, '<internals>');
		}

		if (tag[0] === '#')
		{
			var output = [];
			for (var k in value)
			{
				if (k === '$') continue;
				output.push(_Debug_toAnsiString(ansi, value[k]));
			}
			return '(' + output.join(',') + ')';
		}

		if (tag === 'Set_elm_builtin')
		{
			return _Debug_ctorColor(ansi, 'Set')
				+ _Debug_fadeColor(ansi, '.fromList') + ' '
				+ _Debug_toAnsiString(ansi, $elm$core$Set$toList(value));
		}

		if (tag === 'RBNode_elm_builtin' || tag === 'RBEmpty_elm_builtin')
		{
			return _Debug_ctorColor(ansi, 'Dict')
				+ _Debug_fadeColor(ansi, '.fromList') + ' '
				+ _Debug_toAnsiString(ansi, $elm$core$Dict$toList(value));
		}

		if (tag === 'Array_elm_builtin')
		{
			return _Debug_ctorColor(ansi, 'Array')
				+ _Debug_fadeColor(ansi, '.fromList') + ' '
				+ _Debug_toAnsiString(ansi, $elm$core$Array$toList(value));
		}

		if (tag === '::' || tag === '[]')
		{
			var output = '[';

			value.b && (output += _Debug_toAnsiString(ansi, value.a), value = value.b)

			for (; value.b; value = value.b) // WHILE_CONS
			{
				output += ',' + _Debug_toAnsiString(ansi, value.a);
			}
			return output + ']';
		}

		var output = '';
		for (var i in value)
		{
			if (i === '$') continue;
			var str = _Debug_toAnsiString(ansi, value[i]);
			var c0 = str[0];
			var parenless = c0 === '{' || c0 === '(' || c0 === '[' || c0 === '<' || c0 === '"' || str.indexOf(' ') < 0;
			output += ' ' + (parenless ? str : '(' + str + ')');
		}
		return _Debug_ctorColor(ansi, tag) + output;
	}

	if (typeof DataView === 'function' && value instanceof DataView)
	{
		return _Debug_stringColor(ansi, '<' + value.byteLength + ' bytes>');
	}

	if (typeof File !== 'undefined' && value instanceof File)
	{
		return _Debug_internalColor(ansi, '<' + value.name + '>');
	}

	if (typeof value === 'object')
	{
		var output = [];
		for (var key in value)
		{
			var field = key[0] === '_' ? key.slice(1) : key;
			output.push(_Debug_fadeColor(ansi, field) + ' = ' + _Debug_toAnsiString(ansi, value[key]));
		}
		if (output.length === 0)
		{
			return '{}';
		}
		return '{ ' + output.join(', ') + ' }';
	}

	return _Debug_internalColor(ansi, '<internals>');
}

function _Debug_addSlashes(str, isChar)
{
	var s = str
		.replace(/\\/g, '\\\\')
		.replace(/\n/g, '\\n')
		.replace(/\t/g, '\\t')
		.replace(/\r/g, '\\r')
		.replace(/\v/g, '\\v')
		.replace(/\0/g, '\\0');

	if (isChar)
	{
		return s.replace(/\'/g, '\\\'');
	}
	else
	{
		return s.replace(/\"/g, '\\"');
	}
}

function _Debug_ctorColor(ansi, string)
{
	return ansi ? '\x1b[96m' + string + '\x1b[0m' : string;
}

function _Debug_numberColor(ansi, string)
{
	return ansi ? '\x1b[95m' + string + '\x1b[0m' : string;
}

function _Debug_stringColor(ansi, string)
{
	return ansi ? '\x1b[93m' + string + '\x1b[0m' : string;
}

function _Debug_charColor(ansi, string)
{
	return ansi ? '\x1b[92m' + string + '\x1b[0m' : string;
}

function _Debug_fadeColor(ansi, string)
{
	return ansi ? '\x1b[37m' + string + '\x1b[0m' : string;
}

function _Debug_internalColor(ansi, string)
{
	return ansi ? '\x1b[36m' + string + '\x1b[0m' : string;
}

function _Debug_toHexDigit(n)
{
	return String.fromCharCode(n < 10 ? 48 + n : 55 + n);
}


// CRASH


function _Debug_crash(identifier)
{
	throw new Error('https://github.com/elm/core/blob/1.0.0/hints/' + identifier + '.md');
}


function _Debug_crash_UNUSED(identifier, fact1, fact2, fact3, fact4)
{
	switch(identifier)
	{
		case 0:
			throw new Error('What node should I take over? In JavaScript I need something like:\n\n    Elm.Main.init({\n        node: document.getElementById("elm-node")\n    })\n\nYou need to do this with any Browser.sandbox or Browser.element program.');

		case 1:
			throw new Error('Browser.application programs cannot handle URLs like this:\n\n    ' + document.location.href + '\n\nWhat is the root? The root of your file system? Try looking at this program with `elm reactor` or some other server.');

		case 2:
			var jsonErrorString = fact1;
			throw new Error('Problem with the flags given to your Elm program on initialization.\n\n' + jsonErrorString);

		case 3:
			var portName = fact1;
			throw new Error('There can only be one port named `' + portName + '`, but your program has multiple.');

		case 4:
			var portName = fact1;
			var problem = fact2;
			throw new Error('Trying to send an unexpected type of value through port `' + portName + '`:\n' + problem);

		case 5:
			throw new Error('Trying to use `(==)` on functions.\nThere is no way to know if functions are "the same" in the Elm sense.\nRead more about this at https://package.elm-lang.org/packages/elm/core/latest/Basics#== which describes why it is this way and what the better version will look like.');

		case 6:
			var moduleName = fact1;
			throw new Error('Your page is loading multiple Elm scripts with a module named ' + moduleName + '. Maybe a duplicate script is getting loaded accidentally? If not, rename one of them so I know which is which!');

		case 8:
			var moduleName = fact1;
			var region = fact2;
			var message = fact3;
			throw new Error('TODO in module `' + moduleName + '` ' + _Debug_regionToString(region) + '\n\n' + message);

		case 9:
			var moduleName = fact1;
			var region = fact2;
			var value = fact3;
			var message = fact4;
			throw new Error(
				'TODO in module `' + moduleName + '` from the `case` expression '
				+ _Debug_regionToString(region) + '\n\nIt received the following value:\n\n    '
				+ _Debug_toString(value).replace('\n', '\n    ')
				+ '\n\nBut the branch that handles it says:\n\n    ' + message.replace('\n', '\n    ')
			);

		case 10:
			throw new Error('Bug in https://github.com/elm/virtual-dom/issues');

		case 11:
			throw new Error('Cannot perform mod 0. Division by zero error.');
	}
}

function _Debug_regionToString(region)
{
	if (region.a9.aL === region.bf.aL)
	{
		return 'on line ' + region.a9.aL;
	}
	return 'on lines ' + region.a9.aL + ' through ' + region.bf.aL;
}



// EQUALITY

function _Utils_eq(x, y)
{
	for (
		var pair, stack = [], isEqual = _Utils_eqHelp(x, y, 0, stack);
		isEqual && (pair = stack.pop());
		isEqual = _Utils_eqHelp(pair.a, pair.b, 0, stack)
		)
	{}

	return isEqual;
}

function _Utils_eqHelp(x, y, depth, stack)
{
	if (x === y)
	{
		return true;
	}

	if (typeof x !== 'object' || x === null || y === null)
	{
		typeof x === 'function' && _Debug_crash(5);
		return false;
	}

	if (depth > 100)
	{
		stack.push(_Utils_Tuple2(x,y));
		return true;
	}

	/**_UNUSED/
	if (x.$ === 'Set_elm_builtin')
	{
		x = $elm$core$Set$toList(x);
		y = $elm$core$Set$toList(y);
	}
	if (x.$ === 'RBNode_elm_builtin' || x.$ === 'RBEmpty_elm_builtin')
	{
		x = $elm$core$Dict$toList(x);
		y = $elm$core$Dict$toList(y);
	}
	//*/

	/**/
	if (x.$ < 0)
	{
		x = $elm$core$Dict$toList(x);
		y = $elm$core$Dict$toList(y);
	}
	//*/

	for (var key in x)
	{
		if (!_Utils_eqHelp(x[key], y[key], depth + 1, stack))
		{
			return false;
		}
	}
	return true;
}

var _Utils_equal = F2(_Utils_eq);
var _Utils_notEqual = F2(function(a, b) { return !_Utils_eq(a,b); });



// COMPARISONS

// Code in Generate/JavaScript.hs, Basics.js, and List.js depends on
// the particular integer values assigned to LT, EQ, and GT.

function _Utils_cmp(x, y, ord)
{
	if (typeof x !== 'object')
	{
		return x === y ? /*EQ*/ 0 : x < y ? /*LT*/ -1 : /*GT*/ 1;
	}

	/**_UNUSED/
	if (x instanceof String)
	{
		var a = x.valueOf();
		var b = y.valueOf();
		return a === b ? 0 : a < b ? -1 : 1;
	}
	//*/

	/**/
	if (typeof x.$ === 'undefined')
	//*/
	/**_UNUSED/
	if (x.$[0] === '#')
	//*/
	{
		return (ord = _Utils_cmp(x.a, y.a))
			? ord
			: (ord = _Utils_cmp(x.b, y.b))
				? ord
				: _Utils_cmp(x.c, y.c);
	}

	// traverse conses until end of a list or a mismatch
	for (; x.b && y.b && !(ord = _Utils_cmp(x.a, y.a)); x = x.b, y = y.b) {} // WHILE_CONSES
	return ord || (x.b ? /*GT*/ 1 : y.b ? /*LT*/ -1 : /*EQ*/ 0);
}

var _Utils_lt = F2(function(a, b) { return _Utils_cmp(a, b) < 0; });
var _Utils_le = F2(function(a, b) { return _Utils_cmp(a, b) < 1; });
var _Utils_gt = F2(function(a, b) { return _Utils_cmp(a, b) > 0; });
var _Utils_ge = F2(function(a, b) { return _Utils_cmp(a, b) >= 0; });

var _Utils_compare = F2(function(x, y)
{
	var n = _Utils_cmp(x, y);
	return n < 0 ? $elm$core$Basics$LT : n ? $elm$core$Basics$GT : $elm$core$Basics$EQ;
});


// COMMON VALUES

var _Utils_Tuple0 = 0;
var _Utils_Tuple0_UNUSED = { $: '#0' };

function _Utils_Tuple2(a, b) { return { a: a, b: b }; }
function _Utils_Tuple2_UNUSED(a, b) { return { $: '#2', a: a, b: b }; }

function _Utils_Tuple3(a, b, c) { return { a: a, b: b, c: c }; }
function _Utils_Tuple3_UNUSED(a, b, c) { return { $: '#3', a: a, b: b, c: c }; }

function _Utils_chr(c) { return c; }
function _Utils_chr_UNUSED(c) { return new String(c); }


// RECORDS

function _Utils_update(oldRecord, updatedFields)
{
	var newRecord = {};

	for (var key in oldRecord)
	{
		newRecord[key] = oldRecord[key];
	}

	for (var key in updatedFields)
	{
		newRecord[key] = updatedFields[key];
	}

	return newRecord;
}


// APPEND

var _Utils_append = F2(_Utils_ap);

function _Utils_ap(xs, ys)
{
	// append Strings
	if (typeof xs === 'string')
	{
		return xs + ys;
	}

	// append Lists
	if (!xs.b)
	{
		return ys;
	}
	var root = _List_Cons(xs.a, ys);
	xs = xs.b
	for (var curr = root; xs.b; xs = xs.b) // WHILE_CONS
	{
		curr = curr.b = _List_Cons(xs.a, ys);
	}
	return root;
}



var _List_Nil = { $: 0 };
var _List_Nil_UNUSED = { $: '[]' };

function _List_Cons(hd, tl) { return { $: 1, a: hd, b: tl }; }
function _List_Cons_UNUSED(hd, tl) { return { $: '::', a: hd, b: tl }; }


var _List_cons = F2(_List_Cons);

function _List_fromArray(arr)
{
	var out = _List_Nil;
	for (var i = arr.length; i--; )
	{
		out = _List_Cons(arr[i], out);
	}
	return out;
}

function _List_toArray(xs)
{
	for (var out = []; xs.b; xs = xs.b) // WHILE_CONS
	{
		out.push(xs.a);
	}
	return out;
}

var _List_map2 = F3(function(f, xs, ys)
{
	for (var arr = []; xs.b && ys.b; xs = xs.b, ys = ys.b) // WHILE_CONSES
	{
		arr.push(A2(f, xs.a, ys.a));
	}
	return _List_fromArray(arr);
});

var _List_map3 = F4(function(f, xs, ys, zs)
{
	for (var arr = []; xs.b && ys.b && zs.b; xs = xs.b, ys = ys.b, zs = zs.b) // WHILE_CONSES
	{
		arr.push(A3(f, xs.a, ys.a, zs.a));
	}
	return _List_fromArray(arr);
});

var _List_map4 = F5(function(f, ws, xs, ys, zs)
{
	for (var arr = []; ws.b && xs.b && ys.b && zs.b; ws = ws.b, xs = xs.b, ys = ys.b, zs = zs.b) // WHILE_CONSES
	{
		arr.push(A4(f, ws.a, xs.a, ys.a, zs.a));
	}
	return _List_fromArray(arr);
});

var _List_map5 = F6(function(f, vs, ws, xs, ys, zs)
{
	for (var arr = []; vs.b && ws.b && xs.b && ys.b && zs.b; vs = vs.b, ws = ws.b, xs = xs.b, ys = ys.b, zs = zs.b) // WHILE_CONSES
	{
		arr.push(A5(f, vs.a, ws.a, xs.a, ys.a, zs.a));
	}
	return _List_fromArray(arr);
});

var _List_sortBy = F2(function(f, xs)
{
	return _List_fromArray(_List_toArray(xs).sort(function(a, b) {
		return _Utils_cmp(f(a), f(b));
	}));
});

var _List_sortWith = F2(function(f, xs)
{
	return _List_fromArray(_List_toArray(xs).sort(function(a, b) {
		var ord = A2(f, a, b);
		return ord === $elm$core$Basics$EQ ? 0 : ord === $elm$core$Basics$LT ? -1 : 1;
	}));
});



// MATH

var _Basics_add = F2(function(a, b) { return a + b; });
var _Basics_sub = F2(function(a, b) { return a - b; });
var _Basics_mul = F2(function(a, b) { return a * b; });
var _Basics_fdiv = F2(function(a, b) { return a / b; });
var _Basics_idiv = F2(function(a, b) { return (a / b) | 0; });
var _Basics_pow = F2(Math.pow);

var _Basics_remainderBy = F2(function(b, a) { return a % b; });

// https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/divmodnote-letter.pdf
var _Basics_modBy = F2(function(modulus, x)
{
	var answer = x % modulus;
	return modulus === 0
		? _Debug_crash(11)
		:
	((answer > 0 && modulus < 0) || (answer < 0 && modulus > 0))
		? answer + modulus
		: answer;
});


// TRIGONOMETRY

var _Basics_pi = Math.PI;
var _Basics_e = Math.E;
var _Basics_cos = Math.cos;
var _Basics_sin = Math.sin;
var _Basics_tan = Math.tan;
var _Basics_acos = Math.acos;
var _Basics_asin = Math.asin;
var _Basics_atan = Math.atan;
var _Basics_atan2 = F2(Math.atan2);


// MORE MATH

function _Basics_toFloat(x) { return x; }
function _Basics_truncate(n) { return n | 0; }
function _Basics_isInfinite(n) { return n === Infinity || n === -Infinity; }

var _Basics_ceiling = Math.ceil;
var _Basics_floor = Math.floor;
var _Basics_round = Math.round;
var _Basics_sqrt = Math.sqrt;
var _Basics_log = Math.log;
var _Basics_isNaN = isNaN;


// BOOLEANS

function _Basics_not(bool) { return !bool; }
var _Basics_and = F2(function(a, b) { return a && b; });
var _Basics_or  = F2(function(a, b) { return a || b; });
var _Basics_xor = F2(function(a, b) { return a !== b; });



var _String_cons = F2(function(chr, str)
{
	return chr + str;
});

function _String_uncons(string)
{
	var word = string.charCodeAt(0);
	return !isNaN(word)
		? $elm$core$Maybe$Just(
			0xD800 <= word && word <= 0xDBFF
				? _Utils_Tuple2(_Utils_chr(string[0] + string[1]), string.slice(2))
				: _Utils_Tuple2(_Utils_chr(string[0]), string.slice(1))
		)
		: $elm$core$Maybe$Nothing;
}

var _String_append = F2(function(a, b)
{
	return a + b;
});

function _String_length(str)
{
	return str.length;
}

var _String_map = F2(function(func, string)
{
	var len = string.length;
	var array = new Array(len);
	var i = 0;
	while (i < len)
	{
		var word = string.charCodeAt(i);
		if (0xD800 <= word && word <= 0xDBFF)
		{
			array[i] = func(_Utils_chr(string[i] + string[i+1]));
			i += 2;
			continue;
		}
		array[i] = func(_Utils_chr(string[i]));
		i++;
	}
	return array.join('');
});

var _String_filter = F2(function(isGood, str)
{
	var arr = [];
	var len = str.length;
	var i = 0;
	while (i < len)
	{
		var char = str[i];
		var word = str.charCodeAt(i);
		i++;
		if (0xD800 <= word && word <= 0xDBFF)
		{
			char += str[i];
			i++;
		}

		if (isGood(_Utils_chr(char)))
		{
			arr.push(char);
		}
	}
	return arr.join('');
});

function _String_reverse(str)
{
	var len = str.length;
	var arr = new Array(len);
	var i = 0;
	while (i < len)
	{
		var word = str.charCodeAt(i);
		if (0xD800 <= word && word <= 0xDBFF)
		{
			arr[len - i] = str[i + 1];
			i++;
			arr[len - i] = str[i - 1];
			i++;
		}
		else
		{
			arr[len - i] = str[i];
			i++;
		}
	}
	return arr.join('');
}

var _String_foldl = F3(function(func, state, string)
{
	var len = string.length;
	var i = 0;
	while (i < len)
	{
		var char = string[i];
		var word = string.charCodeAt(i);
		i++;
		if (0xD800 <= word && word <= 0xDBFF)
		{
			char += string[i];
			i++;
		}
		state = A2(func, _Utils_chr(char), state);
	}
	return state;
});

var _String_foldr = F3(function(func, state, string)
{
	var i = string.length;
	while (i--)
	{
		var char = string[i];
		var word = string.charCodeAt(i);
		if (0xDC00 <= word && word <= 0xDFFF)
		{
			i--;
			char = string[i] + char;
		}
		state = A2(func, _Utils_chr(char), state);
	}
	return state;
});

var _String_split = F2(function(sep, str)
{
	return str.split(sep);
});

var _String_join = F2(function(sep, strs)
{
	return strs.join(sep);
});

var _String_slice = F3(function(start, end, str) {
	return str.slice(start, end);
});

function _String_trim(str)
{
	return str.trim();
}

function _String_trimLeft(str)
{
	return str.replace(/^\s+/, '');
}

function _String_trimRight(str)
{
	return str.replace(/\s+$/, '');
}

function _String_words(str)
{
	return _List_fromArray(str.trim().split(/\s+/g));
}

function _String_lines(str)
{
	return _List_fromArray(str.split(/\r\n|\r|\n/g));
}

function _String_toUpper(str)
{
	return str.toUpperCase();
}

function _String_toLower(str)
{
	return str.toLowerCase();
}

var _String_any = F2(function(isGood, string)
{
	var i = string.length;
	while (i--)
	{
		var char = string[i];
		var word = string.charCodeAt(i);
		if (0xDC00 <= word && word <= 0xDFFF)
		{
			i--;
			char = string[i] + char;
		}
		if (isGood(_Utils_chr(char)))
		{
			return true;
		}
	}
	return false;
});

var _String_all = F2(function(isGood, string)
{
	var i = string.length;
	while (i--)
	{
		var char = string[i];
		var word = string.charCodeAt(i);
		if (0xDC00 <= word && word <= 0xDFFF)
		{
			i--;
			char = string[i] + char;
		}
		if (!isGood(_Utils_chr(char)))
		{
			return false;
		}
	}
	return true;
});

var _String_contains = F2(function(sub, str)
{
	return str.indexOf(sub) > -1;
});

var _String_startsWith = F2(function(sub, str)
{
	return str.indexOf(sub) === 0;
});

var _String_endsWith = F2(function(sub, str)
{
	return str.length >= sub.length &&
		str.lastIndexOf(sub) === str.length - sub.length;
});

var _String_indexes = F2(function(sub, str)
{
	var subLen = sub.length;

	if (subLen < 1)
	{
		return _List_Nil;
	}

	var i = 0;
	var is = [];

	while ((i = str.indexOf(sub, i)) > -1)
	{
		is.push(i);
		i = i + subLen;
	}

	return _List_fromArray(is);
});


// TO STRING

function _String_fromNumber(number)
{
	return number + '';
}


// INT CONVERSIONS

function _String_toInt(str)
{
	var total = 0;
	var code0 = str.charCodeAt(0);
	var start = code0 == 0x2B /* + */ || code0 == 0x2D /* - */ ? 1 : 0;

	for (var i = start; i < str.length; ++i)
	{
		var code = str.charCodeAt(i);
		if (code < 0x30 || 0x39 < code)
		{
			return $elm$core$Maybe$Nothing;
		}
		total = 10 * total + code - 0x30;
	}

	return i == start
		? $elm$core$Maybe$Nothing
		: $elm$core$Maybe$Just(code0 == 0x2D ? -total : total);
}


// FLOAT CONVERSIONS

function _String_toFloat(s)
{
	// check if it is a hex, octal, or binary number
	if (s.length === 0 || /[\sxbo]/.test(s))
	{
		return $elm$core$Maybe$Nothing;
	}
	var n = +s;
	// faster isNaN check
	return n === n ? $elm$core$Maybe$Just(n) : $elm$core$Maybe$Nothing;
}

function _String_fromList(chars)
{
	return _List_toArray(chars).join('');
}




function _Char_toCode(char)
{
	var code = char.charCodeAt(0);
	if (0xD800 <= code && code <= 0xDBFF)
	{
		return (code - 0xD800) * 0x400 + char.charCodeAt(1) - 0xDC00 + 0x10000
	}
	return code;
}

function _Char_fromCode(code)
{
	return _Utils_chr(
		(code < 0 || 0x10FFFF < code)
			? '\uFFFD'
			:
		(code <= 0xFFFF)
			? String.fromCharCode(code)
			:
		(code -= 0x10000,
			String.fromCharCode(Math.floor(code / 0x400) + 0xD800, code % 0x400 + 0xDC00)
		)
	);
}

function _Char_toUpper(char)
{
	return _Utils_chr(char.toUpperCase());
}

function _Char_toLower(char)
{
	return _Utils_chr(char.toLowerCase());
}

function _Char_toLocaleUpper(char)
{
	return _Utils_chr(char.toLocaleUpperCase());
}

function _Char_toLocaleLower(char)
{
	return _Utils_chr(char.toLocaleLowerCase());
}



/**_UNUSED/
function _Json_errorToString(error)
{
	return $elm$json$Json$Decode$errorToString(error);
}
//*/


// CORE DECODERS

function _Json_succeed(msg)
{
	return {
		$: 0,
		a: msg
	};
}

function _Json_fail(msg)
{
	return {
		$: 1,
		a: msg
	};
}

function _Json_decodePrim(decoder)
{
	return { $: 2, b: decoder };
}

var _Json_decodeInt = _Json_decodePrim(function(value) {
	return (typeof value !== 'number')
		? _Json_expecting('an INT', value)
		:
	(-2147483647 < value && value < 2147483647 && (value | 0) === value)
		? $elm$core$Result$Ok(value)
		:
	(isFinite(value) && !(value % 1))
		? $elm$core$Result$Ok(value)
		: _Json_expecting('an INT', value);
});

var _Json_decodeBool = _Json_decodePrim(function(value) {
	return (typeof value === 'boolean')
		? $elm$core$Result$Ok(value)
		: _Json_expecting('a BOOL', value);
});

var _Json_decodeFloat = _Json_decodePrim(function(value) {
	return (typeof value === 'number')
		? $elm$core$Result$Ok(value)
		: _Json_expecting('a FLOAT', value);
});

var _Json_decodeValue = _Json_decodePrim(function(value) {
	return $elm$core$Result$Ok(_Json_wrap(value));
});

var _Json_decodeString = _Json_decodePrim(function(value) {
	return (typeof value === 'string')
		? $elm$core$Result$Ok(value)
		: (value instanceof String)
			? $elm$core$Result$Ok(value + '')
			: _Json_expecting('a STRING', value);
});

function _Json_decodeList(decoder) { return { $: 3, b: decoder }; }
function _Json_decodeArray(decoder) { return { $: 4, b: decoder }; }

function _Json_decodeNull(value) { return { $: 5, c: value }; }

var _Json_decodeField = F2(function(field, decoder)
{
	return {
		$: 6,
		d: field,
		b: decoder
	};
});

var _Json_decodeIndex = F2(function(index, decoder)
{
	return {
		$: 7,
		e: index,
		b: decoder
	};
});

function _Json_decodeKeyValuePairs(decoder)
{
	return {
		$: 8,
		b: decoder
	};
}

function _Json_mapMany(f, decoders)
{
	return {
		$: 9,
		f: f,
		g: decoders
	};
}

var _Json_andThen = F2(function(callback, decoder)
{
	return {
		$: 10,
		b: decoder,
		h: callback
	};
});

function _Json_oneOf(decoders)
{
	return {
		$: 11,
		g: decoders
	};
}


// DECODING OBJECTS

var _Json_map1 = F2(function(f, d1)
{
	return _Json_mapMany(f, [d1]);
});

var _Json_map2 = F3(function(f, d1, d2)
{
	return _Json_mapMany(f, [d1, d2]);
});

var _Json_map3 = F4(function(f, d1, d2, d3)
{
	return _Json_mapMany(f, [d1, d2, d3]);
});

var _Json_map4 = F5(function(f, d1, d2, d3, d4)
{
	return _Json_mapMany(f, [d1, d2, d3, d4]);
});

var _Json_map5 = F6(function(f, d1, d2, d3, d4, d5)
{
	return _Json_mapMany(f, [d1, d2, d3, d4, d5]);
});

var _Json_map6 = F7(function(f, d1, d2, d3, d4, d5, d6)
{
	return _Json_mapMany(f, [d1, d2, d3, d4, d5, d6]);
});

var _Json_map7 = F8(function(f, d1, d2, d3, d4, d5, d6, d7)
{
	return _Json_mapMany(f, [d1, d2, d3, d4, d5, d6, d7]);
});

var _Json_map8 = F9(function(f, d1, d2, d3, d4, d5, d6, d7, d8)
{
	return _Json_mapMany(f, [d1, d2, d3, d4, d5, d6, d7, d8]);
});


// DECODE

var _Json_runOnString = F2(function(decoder, string)
{
	try
	{
		var value = JSON.parse(string);
		return _Json_runHelp(decoder, value);
	}
	catch (e)
	{
		return $elm$core$Result$Err(A2($elm$json$Json$Decode$Failure, 'This is not valid JSON! ' + e.message, _Json_wrap(string)));
	}
});

var _Json_run = F2(function(decoder, value)
{
	return _Json_runHelp(decoder, _Json_unwrap(value));
});

function _Json_runHelp(decoder, value)
{
	switch (decoder.$)
	{
		case 2:
			return decoder.b(value);

		case 5:
			return (value === null)
				? $elm$core$Result$Ok(decoder.c)
				: _Json_expecting('null', value);

		case 3:
			if (!_Json_isArray(value))
			{
				return _Json_expecting('a LIST', value);
			}
			return _Json_runArrayDecoder(decoder.b, value, _List_fromArray);

		case 4:
			if (!_Json_isArray(value))
			{
				return _Json_expecting('an ARRAY', value);
			}
			return _Json_runArrayDecoder(decoder.b, value, _Json_toElmArray);

		case 6:
			var field = decoder.d;
			if (typeof value !== 'object' || value === null || !(field in value))
			{
				return _Json_expecting('an OBJECT with a field named `' + field + '`', value);
			}
			var result = _Json_runHelp(decoder.b, value[field]);
			return ($elm$core$Result$isOk(result)) ? result : $elm$core$Result$Err(A2($elm$json$Json$Decode$Field, field, result.a));

		case 7:
			var index = decoder.e;
			if (!_Json_isArray(value))
			{
				return _Json_expecting('an ARRAY', value);
			}
			if (index >= value.length)
			{
				return _Json_expecting('a LONGER array. Need index ' + index + ' but only see ' + value.length + ' entries', value);
			}
			var result = _Json_runHelp(decoder.b, value[index]);
			return ($elm$core$Result$isOk(result)) ? result : $elm$core$Result$Err(A2($elm$json$Json$Decode$Index, index, result.a));

		case 8:
			if (typeof value !== 'object' || value === null || _Json_isArray(value))
			{
				return _Json_expecting('an OBJECT', value);
			}

			var keyValuePairs = _List_Nil;
			// TODO test perf of Object.keys and switch when support is good enough
			for (var key in value)
			{
				if (value.hasOwnProperty(key))
				{
					var result = _Json_runHelp(decoder.b, value[key]);
					if (!$elm$core$Result$isOk(result))
					{
						return $elm$core$Result$Err(A2($elm$json$Json$Decode$Field, key, result.a));
					}
					keyValuePairs = _List_Cons(_Utils_Tuple2(key, result.a), keyValuePairs);
				}
			}
			return $elm$core$Result$Ok($elm$core$List$reverse(keyValuePairs));

		case 9:
			var answer = decoder.f;
			var decoders = decoder.g;
			for (var i = 0; i < decoders.length; i++)
			{
				var result = _Json_runHelp(decoders[i], value);
				if (!$elm$core$Result$isOk(result))
				{
					return result;
				}
				answer = answer(result.a);
			}
			return $elm$core$Result$Ok(answer);

		case 10:
			var result = _Json_runHelp(decoder.b, value);
			return (!$elm$core$Result$isOk(result))
				? result
				: _Json_runHelp(decoder.h(result.a), value);

		case 11:
			var errors = _List_Nil;
			for (var temp = decoder.g; temp.b; temp = temp.b) // WHILE_CONS
			{
				var result = _Json_runHelp(temp.a, value);
				if ($elm$core$Result$isOk(result))
				{
					return result;
				}
				errors = _List_Cons(result.a, errors);
			}
			return $elm$core$Result$Err($elm$json$Json$Decode$OneOf($elm$core$List$reverse(errors)));

		case 1:
			return $elm$core$Result$Err(A2($elm$json$Json$Decode$Failure, decoder.a, _Json_wrap(value)));

		case 0:
			return $elm$core$Result$Ok(decoder.a);
	}
}

function _Json_runArrayDecoder(decoder, value, toElmValue)
{
	var len = value.length;
	var array = new Array(len);
	for (var i = 0; i < len; i++)
	{
		var result = _Json_runHelp(decoder, value[i]);
		if (!$elm$core$Result$isOk(result))
		{
			return $elm$core$Result$Err(A2($elm$json$Json$Decode$Index, i, result.a));
		}
		array[i] = result.a;
	}
	return $elm$core$Result$Ok(toElmValue(array));
}

function _Json_isArray(value)
{
	return Array.isArray(value) || (typeof FileList !== 'undefined' && value instanceof FileList);
}

function _Json_toElmArray(array)
{
	return A2($elm$core$Array$initialize, array.length, function(i) { return array[i]; });
}

function _Json_expecting(type, value)
{
	return $elm$core$Result$Err(A2($elm$json$Json$Decode$Failure, 'Expecting ' + type, _Json_wrap(value)));
}


// EQUALITY

function _Json_equality(x, y)
{
	if (x === y)
	{
		return true;
	}

	if (x.$ !== y.$)
	{
		return false;
	}

	switch (x.$)
	{
		case 0:
		case 1:
			return x.a === y.a;

		case 2:
			return x.b === y.b;

		case 5:
			return x.c === y.c;

		case 3:
		case 4:
		case 8:
			return _Json_equality(x.b, y.b);

		case 6:
			return x.d === y.d && _Json_equality(x.b, y.b);

		case 7:
			return x.e === y.e && _Json_equality(x.b, y.b);

		case 9:
			return x.f === y.f && _Json_listEquality(x.g, y.g);

		case 10:
			return x.h === y.h && _Json_equality(x.b, y.b);

		case 11:
			return _Json_listEquality(x.g, y.g);
	}
}

function _Json_listEquality(aDecoders, bDecoders)
{
	var len = aDecoders.length;
	if (len !== bDecoders.length)
	{
		return false;
	}
	for (var i = 0; i < len; i++)
	{
		if (!_Json_equality(aDecoders[i], bDecoders[i]))
		{
			return false;
		}
	}
	return true;
}


// ENCODE

var _Json_encode = F2(function(indentLevel, value)
{
	return JSON.stringify(_Json_unwrap(value), null, indentLevel) + '';
});

function _Json_wrap_UNUSED(value) { return { $: 0, a: value }; }
function _Json_unwrap_UNUSED(value) { return value.a; }

function _Json_wrap(value) { return value; }
function _Json_unwrap(value) { return value; }

function _Json_emptyArray() { return []; }
function _Json_emptyObject() { return {}; }

var _Json_addField = F3(function(key, value, object)
{
	object[key] = _Json_unwrap(value);
	return object;
});

function _Json_addEntry(func)
{
	return F2(function(entry, array)
	{
		array.push(_Json_unwrap(func(entry)));
		return array;
	});
}

var _Json_encodeNull = _Json_wrap(null);



// TASKS

function _Scheduler_succeed(value)
{
	return {
		$: 0,
		a: value
	};
}

function _Scheduler_fail(error)
{
	return {
		$: 1,
		a: error
	};
}

function _Scheduler_binding(callback)
{
	return {
		$: 2,
		b: callback,
		c: null
	};
}

var _Scheduler_andThen = F2(function(callback, task)
{
	return {
		$: 3,
		b: callback,
		d: task
	};
});

var _Scheduler_onError = F2(function(callback, task)
{
	return {
		$: 4,
		b: callback,
		d: task
	};
});

function _Scheduler_receive(callback)
{
	return {
		$: 5,
		b: callback
	};
}


// PROCESSES

var _Scheduler_guid = 0;

function _Scheduler_rawSpawn(task)
{
	var proc = {
		$: 0,
		e: _Scheduler_guid++,
		f: task,
		g: null,
		h: []
	};

	_Scheduler_enqueue(proc);

	return proc;
}

function _Scheduler_spawn(task)
{
	return _Scheduler_binding(function(callback) {
		callback(_Scheduler_succeed(_Scheduler_rawSpawn(task)));
	});
}

function _Scheduler_rawSend(proc, msg)
{
	proc.h.push(msg);
	_Scheduler_enqueue(proc);
}

var _Scheduler_send = F2(function(proc, msg)
{
	return _Scheduler_binding(function(callback) {
		_Scheduler_rawSend(proc, msg);
		callback(_Scheduler_succeed(_Utils_Tuple0));
	});
});

function _Scheduler_kill(proc)
{
	return _Scheduler_binding(function(callback) {
		var task = proc.f;
		if (task.$ === 2 && task.c)
		{
			task.c();
		}

		proc.f = null;

		callback(_Scheduler_succeed(_Utils_Tuple0));
	});
}


/* STEP PROCESSES

type alias Process =
  { $ : tag
  , id : unique_id
  , root : Task
  , stack : null | { $: SUCCEED | FAIL, a: callback, b: stack }
  , mailbox : [msg]
  }

*/


var _Scheduler_working = false;
var _Scheduler_queue = [];


function _Scheduler_enqueue(proc)
{
	_Scheduler_queue.push(proc);
	if (_Scheduler_working)
	{
		return;
	}
	_Scheduler_working = true;
	while (proc = _Scheduler_queue.shift())
	{
		_Scheduler_step(proc);
	}
	_Scheduler_working = false;
}


function _Scheduler_step(proc)
{
	while (proc.f)
	{
		var rootTag = proc.f.$;
		if (rootTag === 0 || rootTag === 1)
		{
			while (proc.g && proc.g.$ !== rootTag)
			{
				proc.g = proc.g.i;
			}
			if (!proc.g)
			{
				return;
			}
			proc.f = proc.g.b(proc.f.a);
			proc.g = proc.g.i;
		}
		else if (rootTag === 2)
		{
			proc.f.c = proc.f.b(function(newRoot) {
				proc.f = newRoot;
				_Scheduler_enqueue(proc);
			});
			return;
		}
		else if (rootTag === 5)
		{
			if (proc.h.length === 0)
			{
				return;
			}
			proc.f = proc.f.b(proc.h.shift());
		}
		else // if (rootTag === 3 || rootTag === 4)
		{
			proc.g = {
				$: rootTag === 3 ? 0 : 1,
				b: proc.f.b,
				i: proc.g
			};
			proc.f = proc.f.d;
		}
	}
}



function _Process_sleep(time)
{
	return _Scheduler_binding(function(callback) {
		var id = setTimeout(function() {
			callback(_Scheduler_succeed(_Utils_Tuple0));
		}, time);

		return function() { clearTimeout(id); };
	});
}




// PROGRAMS


var _Platform_worker = F4(function(impl, flagDecoder, debugMetadata, args)
{
	return _Platform_initialize(
		flagDecoder,
		args,
		impl.bU,
		impl.b5,
		impl.b3,
		function() { return function() {} }
	);
});



// INITIALIZE A PROGRAM


function _Platform_initialize(flagDecoder, args, init, update, subscriptions, stepperBuilder)
{
	var result = A2(_Json_run, flagDecoder, _Json_wrap(args ? args['flags'] : undefined));
	$elm$core$Result$isOk(result) || _Debug_crash(2 /**_UNUSED/, _Json_errorToString(result.a) /**/);
	var managers = {};
	var initPair = init(result.a);
	var model = initPair.a;
	var stepper = stepperBuilder(sendToApp, model);
	var ports = _Platform_setupEffects(managers, sendToApp);

	function sendToApp(msg, viewMetadata)
	{
		var pair = A2(update, msg, model);
		stepper(model = pair.a, viewMetadata);
		_Platform_enqueueEffects(managers, pair.b, subscriptions(model));
	}

	_Platform_enqueueEffects(managers, initPair.b, subscriptions(model));

	return ports ? { ports: ports } : {};
}



// TRACK PRELOADS
//
// This is used by code in elm/browser and elm/http
// to register any HTTP requests that are triggered by init.
//


var _Platform_preload;


function _Platform_registerPreload(url)
{
	_Platform_preload.add(url);
}



// EFFECT MANAGERS


var _Platform_effectManagers = {};


function _Platform_setupEffects(managers, sendToApp)
{
	var ports;

	// setup all necessary effect managers
	for (var key in _Platform_effectManagers)
	{
		var manager = _Platform_effectManagers[key];

		if (manager.a)
		{
			ports = ports || {};
			ports[key] = manager.a(key, sendToApp);
		}

		managers[key] = _Platform_instantiateManager(manager, sendToApp);
	}

	return ports;
}


function _Platform_createManager(init, onEffects, onSelfMsg, cmdMap, subMap)
{
	return {
		b: init,
		c: onEffects,
		d: onSelfMsg,
		e: cmdMap,
		f: subMap
	};
}


function _Platform_instantiateManager(info, sendToApp)
{
	var router = {
		g: sendToApp,
		h: undefined
	};

	var onEffects = info.c;
	var onSelfMsg = info.d;
	var cmdMap = info.e;
	var subMap = info.f;

	function loop(state)
	{
		return A2(_Scheduler_andThen, loop, _Scheduler_receive(function(msg)
		{
			var value = msg.a;

			if (msg.$ === 0)
			{
				return A3(onSelfMsg, router, value, state);
			}

			return cmdMap && subMap
				? A4(onEffects, router, value.i, value.j, state)
				: A3(onEffects, router, cmdMap ? value.i : value.j, state);
		}));
	}

	return router.h = _Scheduler_rawSpawn(A2(_Scheduler_andThen, loop, info.b));
}



// ROUTING


var _Platform_sendToApp = F2(function(router, msg)
{
	return _Scheduler_binding(function(callback)
	{
		router.g(msg);
		callback(_Scheduler_succeed(_Utils_Tuple0));
	});
});


var _Platform_sendToSelf = F2(function(router, msg)
{
	return A2(_Scheduler_send, router.h, {
		$: 0,
		a: msg
	});
});



// BAGS


function _Platform_leaf(home)
{
	return function(value)
	{
		return {
			$: 1,
			k: home,
			l: value
		};
	};
}


function _Platform_batch(list)
{
	return {
		$: 2,
		m: list
	};
}


var _Platform_map = F2(function(tagger, bag)
{
	return {
		$: 3,
		n: tagger,
		o: bag
	}
});



// PIPE BAGS INTO EFFECT MANAGERS
//
// Effects must be queued!
//
// Say your init contains a synchronous command, like Time.now or Time.here
//
//   - This will produce a batch of effects (FX_1)
//   - The synchronous task triggers the subsequent `update` call
//   - This will produce a batch of effects (FX_2)
//
// If we just start dispatching FX_2, subscriptions from FX_2 can be processed
// before subscriptions from FX_1. No good! Earlier versions of this code had
// this problem, leading to these reports:
//
//   https://github.com/elm/core/issues/980
//   https://github.com/elm/core/pull/981
//   https://github.com/elm/compiler/issues/1776
//
// The queue is necessary to avoid ordering issues for synchronous commands.


// Why use true/false here? Why not just check the length of the queue?
// The goal is to detect "are we currently dispatching effects?" If we
// are, we need to bail and let the ongoing while loop handle things.
//
// Now say the queue has 1 element. When we dequeue the final element,
// the queue will be empty, but we are still actively dispatching effects.
// So you could get queue jumping in a really tricky category of cases.
//
var _Platform_effectsQueue = [];
var _Platform_effectsActive = false;


function _Platform_enqueueEffects(managers, cmdBag, subBag)
{
	_Platform_effectsQueue.push({ p: managers, q: cmdBag, r: subBag });

	if (_Platform_effectsActive) return;

	_Platform_effectsActive = true;
	for (var fx; fx = _Platform_effectsQueue.shift(); )
	{
		_Platform_dispatchEffects(fx.p, fx.q, fx.r);
	}
	_Platform_effectsActive = false;
}


function _Platform_dispatchEffects(managers, cmdBag, subBag)
{
	var effectsDict = {};
	_Platform_gatherEffects(true, cmdBag, effectsDict, null);
	_Platform_gatherEffects(false, subBag, effectsDict, null);

	for (var home in managers)
	{
		_Scheduler_rawSend(managers[home], {
			$: 'fx',
			a: effectsDict[home] || { i: _List_Nil, j: _List_Nil }
		});
	}
}


function _Platform_gatherEffects(isCmd, bag, effectsDict, taggers)
{
	switch (bag.$)
	{
		case 1:
			var home = bag.k;
			var effect = _Platform_toEffect(isCmd, home, taggers, bag.l);
			effectsDict[home] = _Platform_insert(isCmd, effect, effectsDict[home]);
			return;

		case 2:
			for (var list = bag.m; list.b; list = list.b) // WHILE_CONS
			{
				_Platform_gatherEffects(isCmd, list.a, effectsDict, taggers);
			}
			return;

		case 3:
			_Platform_gatherEffects(isCmd, bag.o, effectsDict, {
				s: bag.n,
				t: taggers
			});
			return;
	}
}


function _Platform_toEffect(isCmd, home, taggers, value)
{
	function applyTaggers(x)
	{
		for (var temp = taggers; temp; temp = temp.t)
		{
			x = temp.s(x);
		}
		return x;
	}

	var map = isCmd
		? _Platform_effectManagers[home].e
		: _Platform_effectManagers[home].f;

	return A2(map, applyTaggers, value)
}


function _Platform_insert(isCmd, newEffect, effects)
{
	effects = effects || { i: _List_Nil, j: _List_Nil };

	isCmd
		? (effects.i = _List_Cons(newEffect, effects.i))
		: (effects.j = _List_Cons(newEffect, effects.j));

	return effects;
}



// PORTS


function _Platform_checkPortName(name)
{
	if (_Platform_effectManagers[name])
	{
		_Debug_crash(3, name)
	}
}



// OUTGOING PORTS


function _Platform_outgoingPort(name, converter)
{
	_Platform_checkPortName(name);
	_Platform_effectManagers[name] = {
		e: _Platform_outgoingPortMap,
		u: converter,
		a: _Platform_setupOutgoingPort
	};
	return _Platform_leaf(name);
}


var _Platform_outgoingPortMap = F2(function(tagger, value) { return value; });


function _Platform_setupOutgoingPort(name)
{
	var subs = [];
	var converter = _Platform_effectManagers[name].u;

	// CREATE MANAGER

	var init = _Process_sleep(0);

	_Platform_effectManagers[name].b = init;
	_Platform_effectManagers[name].c = F3(function(router, cmdList, state)
	{
		for ( ; cmdList.b; cmdList = cmdList.b) // WHILE_CONS
		{
			// grab a separate reference to subs in case unsubscribe is called
			var currentSubs = subs;
			var value = _Json_unwrap(converter(cmdList.a));
			for (var i = 0; i < currentSubs.length; i++)
			{
				currentSubs[i](value);
			}
		}
		return init;
	});

	// PUBLIC API

	function subscribe(callback)
	{
		subs.push(callback);
	}

	function unsubscribe(callback)
	{
		// copy subs into a new array in case unsubscribe is called within a
		// subscribed callback
		subs = subs.slice();
		var index = subs.indexOf(callback);
		if (index >= 0)
		{
			subs.splice(index, 1);
		}
	}

	return {
		subscribe: subscribe,
		unsubscribe: unsubscribe
	};
}



// INCOMING PORTS


function _Platform_incomingPort(name, converter)
{
	_Platform_checkPortName(name);
	_Platform_effectManagers[name] = {
		f: _Platform_incomingPortMap,
		u: converter,
		a: _Platform_setupIncomingPort
	};
	return _Platform_leaf(name);
}


var _Platform_incomingPortMap = F2(function(tagger, finalTagger)
{
	return function(value)
	{
		return tagger(finalTagger(value));
	};
});


function _Platform_setupIncomingPort(name, sendToApp)
{
	var subs = _List_Nil;
	var converter = _Platform_effectManagers[name].u;

	// CREATE MANAGER

	var init = _Scheduler_succeed(null);

	_Platform_effectManagers[name].b = init;
	_Platform_effectManagers[name].c = F3(function(router, subList, state)
	{
		subs = subList;
		return init;
	});

	// PUBLIC API

	function send(incomingValue)
	{
		var result = A2(_Json_run, converter, _Json_wrap(incomingValue));

		$elm$core$Result$isOk(result) || _Debug_crash(4, name, result.a);

		var value = result.a;
		for (var temp = subs; temp.b; temp = temp.b) // WHILE_CONS
		{
			sendToApp(temp.a(value));
		}
	}

	return { send: send };
}



// EXPORT ELM MODULES
//
// Have DEBUG and PROD versions so that we can (1) give nicer errors in
// debug mode and (2) not pay for the bits needed for that in prod mode.
//


function _Platform_export(exports)
{
	scope['Elm']
		? _Platform_mergeExportsProd(scope['Elm'], exports)
		: scope['Elm'] = exports;
}


function _Platform_mergeExportsProd(obj, exports)
{
	for (var name in exports)
	{
		(name in obj)
			? (name == 'init')
				? _Debug_crash(6)
				: _Platform_mergeExportsProd(obj[name], exports[name])
			: (obj[name] = exports[name]);
	}
}


function _Platform_export_UNUSED(exports)
{
	scope['Elm']
		? _Platform_mergeExportsDebug('Elm', scope['Elm'], exports)
		: scope['Elm'] = exports;
}


function _Platform_mergeExportsDebug(moduleName, obj, exports)
{
	for (var name in exports)
	{
		(name in obj)
			? (name == 'init')
				? _Debug_crash(6, moduleName)
				: _Platform_mergeExportsDebug(moduleName + '.' + name, obj[name], exports[name])
			: (obj[name] = exports[name]);
	}
}




// HELPERS


var _VirtualDom_divertHrefToApp;

var _VirtualDom_doc = typeof document !== 'undefined' ? document : {};


function _VirtualDom_appendChild(parent, child)
{
	parent.appendChild(child);
}

var _VirtualDom_init = F4(function(virtualNode, flagDecoder, debugMetadata, args)
{
	// NOTE: this function needs _Platform_export available to work

	/**/
	var node = args['node'];
	//*/
	/**_UNUSED/
	var node = args && args['node'] ? args['node'] : _Debug_crash(0);
	//*/

	node.parentNode.replaceChild(
		_VirtualDom_render(virtualNode, function() {}),
		node
	);

	return {};
});



// TEXT


function _VirtualDom_text(string)
{
	return {
		$: 0,
		a: string
	};
}



// NODE


var _VirtualDom_nodeNS = F2(function(namespace, tag)
{
	return F2(function(factList, kidList)
	{
		for (var kids = [], descendantsCount = 0; kidList.b; kidList = kidList.b) // WHILE_CONS
		{
			var kid = kidList.a;
			descendantsCount += (kid.b || 0);
			kids.push(kid);
		}
		descendantsCount += kids.length;

		return {
			$: 1,
			c: tag,
			d: _VirtualDom_organizeFacts(factList),
			e: kids,
			f: namespace,
			b: descendantsCount
		};
	});
});


var _VirtualDom_node = _VirtualDom_nodeNS(undefined);



// KEYED NODE


var _VirtualDom_keyedNodeNS = F2(function(namespace, tag)
{
	return F2(function(factList, kidList)
	{
		for (var kids = [], descendantsCount = 0; kidList.b; kidList = kidList.b) // WHILE_CONS
		{
			var kid = kidList.a;
			descendantsCount += (kid.b.b || 0);
			kids.push(kid);
		}
		descendantsCount += kids.length;

		return {
			$: 2,
			c: tag,
			d: _VirtualDom_organizeFacts(factList),
			e: kids,
			f: namespace,
			b: descendantsCount
		};
	});
});


var _VirtualDom_keyedNode = _VirtualDom_keyedNodeNS(undefined);



// CUSTOM


function _VirtualDom_custom(factList, model, render, diff)
{
	return {
		$: 3,
		d: _VirtualDom_organizeFacts(factList),
		g: model,
		h: render,
		i: diff
	};
}



// MAP


var _VirtualDom_map = F2(function(tagger, node)
{
	return {
		$: 4,
		j: tagger,
		k: node,
		b: 1 + (node.b || 0)
	};
});



// LAZY


function _VirtualDom_thunk(refs, thunk)
{
	return {
		$: 5,
		l: refs,
		m: thunk,
		k: undefined
	};
}

var _VirtualDom_lazy = F2(function(func, a)
{
	return _VirtualDom_thunk([func, a], function() {
		return func(a);
	});
});

var _VirtualDom_lazy2 = F3(function(func, a, b)
{
	return _VirtualDom_thunk([func, a, b], function() {
		return A2(func, a, b);
	});
});

var _VirtualDom_lazy3 = F4(function(func, a, b, c)
{
	return _VirtualDom_thunk([func, a, b, c], function() {
		return A3(func, a, b, c);
	});
});

var _VirtualDom_lazy4 = F5(function(func, a, b, c, d)
{
	return _VirtualDom_thunk([func, a, b, c, d], function() {
		return A4(func, a, b, c, d);
	});
});

var _VirtualDom_lazy5 = F6(function(func, a, b, c, d, e)
{
	return _VirtualDom_thunk([func, a, b, c, d, e], function() {
		return A5(func, a, b, c, d, e);
	});
});

var _VirtualDom_lazy6 = F7(function(func, a, b, c, d, e, f)
{
	return _VirtualDom_thunk([func, a, b, c, d, e, f], function() {
		return A6(func, a, b, c, d, e, f);
	});
});

var _VirtualDom_lazy7 = F8(function(func, a, b, c, d, e, f, g)
{
	return _VirtualDom_thunk([func, a, b, c, d, e, f, g], function() {
		return A7(func, a, b, c, d, e, f, g);
	});
});

var _VirtualDom_lazy8 = F9(function(func, a, b, c, d, e, f, g, h)
{
	return _VirtualDom_thunk([func, a, b, c, d, e, f, g, h], function() {
		return A8(func, a, b, c, d, e, f, g, h);
	});
});



// FACTS


var _VirtualDom_on = F2(function(key, handler)
{
	return {
		$: 'a0',
		n: key,
		o: handler
	};
});
var _VirtualDom_style = F2(function(key, value)
{
	return {
		$: 'a1',
		n: key,
		o: value
	};
});
var _VirtualDom_property = F2(function(key, value)
{
	return {
		$: 'a2',
		n: key,
		o: value
	};
});
var _VirtualDom_attribute = F2(function(key, value)
{
	return {
		$: 'a3',
		n: key,
		o: value
	};
});
var _VirtualDom_attributeNS = F3(function(namespace, key, value)
{
	return {
		$: 'a4',
		n: key,
		o: { f: namespace, o: value }
	};
});



// XSS ATTACK VECTOR CHECKS
//
// For some reason, tabs can appear in href protocols and it still works.
// So '\tjava\tSCRIPT:alert("!!!")' and 'javascript:alert("!!!")' are the same
// in practice. That is why _VirtualDom_RE_js and _VirtualDom_RE_js_html look
// so freaky.
//
// Pulling the regular expressions out to the top level gives a slight speed
// boost in small benchmarks (4-10%) but hoisting values to reduce allocation
// can be unpredictable in large programs where JIT may have a harder time with
// functions are not fully self-contained. The benefit is more that the js and
// js_html ones are so weird that I prefer to see them near each other.


var _VirtualDom_RE_script = /^script$/i;
var _VirtualDom_RE_on_formAction = /^(on|formAction$)/i;
var _VirtualDom_RE_js = /^\s*j\s*a\s*v\s*a\s*s\s*c\s*r\s*i\s*p\s*t\s*:/i;
var _VirtualDom_RE_js_html = /^\s*(j\s*a\s*v\s*a\s*s\s*c\s*r\s*i\s*p\s*t\s*:|d\s*a\s*t\s*a\s*:\s*t\s*e\s*x\s*t\s*\/\s*h\s*t\s*m\s*l\s*(,|;))/i;


function _VirtualDom_noScript(tag)
{
	return _VirtualDom_RE_script.test(tag) ? 'p' : tag;
}

function _VirtualDom_noOnOrFormAction(key)
{
	return _VirtualDom_RE_on_formAction.test(key) ? 'data-' + key : key;
}

function _VirtualDom_noInnerHtmlOrFormAction(key)
{
	return key == 'innerHTML' || key == 'formAction' ? 'data-' + key : key;
}

function _VirtualDom_noJavaScriptUri(value)
{
	return _VirtualDom_RE_js.test(value)
		? /**/''//*//**_UNUSED/'javascript:alert("This is an XSS vector. Please use ports or web components instead.")'//*/
		: value;
}

function _VirtualDom_noJavaScriptOrHtmlUri(value)
{
	return _VirtualDom_RE_js_html.test(value)
		? /**/''//*//**_UNUSED/'javascript:alert("This is an XSS vector. Please use ports or web components instead.")'//*/
		: value;
}

function _VirtualDom_noJavaScriptOrHtmlJson(value)
{
	return (typeof _Json_unwrap(value) === 'string' && _VirtualDom_RE_js_html.test(_Json_unwrap(value)))
		? _Json_wrap(
			/**/''//*//**_UNUSED/'javascript:alert("This is an XSS vector. Please use ports or web components instead.")'//*/
		) : value;
}



// MAP FACTS


var _VirtualDom_mapAttribute = F2(function(func, attr)
{
	return (attr.$ === 'a0')
		? A2(_VirtualDom_on, attr.n, _VirtualDom_mapHandler(func, attr.o))
		: attr;
});

function _VirtualDom_mapHandler(func, handler)
{
	var tag = $elm$virtual_dom$VirtualDom$toHandlerInt(handler);

	// 0 = Normal
	// 1 = MayStopPropagation
	// 2 = MayPreventDefault
	// 3 = Custom

	return {
		$: handler.$,
		a:
			!tag
				? A2($elm$json$Json$Decode$map, func, handler.a)
				:
			A3($elm$json$Json$Decode$map2,
				tag < 3
					? _VirtualDom_mapEventTuple
					: _VirtualDom_mapEventRecord,
				$elm$json$Json$Decode$succeed(func),
				handler.a
			)
	};
}

var _VirtualDom_mapEventTuple = F2(function(func, tuple)
{
	return _Utils_Tuple2(func(tuple.a), tuple.b);
});

var _VirtualDom_mapEventRecord = F2(function(func, record)
{
	return {
		aa: func(record.aa),
		ba: record.ba,
		a7: record.a7
	}
});



// ORGANIZE FACTS


function _VirtualDom_organizeFacts(factList)
{
	for (var facts = {}; factList.b; factList = factList.b) // WHILE_CONS
	{
		var entry = factList.a;

		var tag = entry.$;
		var key = entry.n;
		var value = entry.o;

		if (tag === 'a2')
		{
			(key === 'className')
				? _VirtualDom_addClass(facts, key, _Json_unwrap(value))
				: facts[key] = _Json_unwrap(value);

			continue;
		}

		var subFacts = facts[tag] || (facts[tag] = {});
		(tag === 'a3' && key === 'class')
			? _VirtualDom_addClass(subFacts, key, value)
			: subFacts[key] = value;
	}

	return facts;
}

function _VirtualDom_addClass(object, key, newClass)
{
	var classes = object[key];
	object[key] = classes ? classes + ' ' + newClass : newClass;
}



// RENDER


function _VirtualDom_render(vNode, eventNode)
{
	var tag = vNode.$;

	if (tag === 5)
	{
		return _VirtualDom_render(vNode.k || (vNode.k = vNode.m()), eventNode);
	}

	if (tag === 0)
	{
		return _VirtualDom_doc.createTextNode(vNode.a);
	}

	if (tag === 4)
	{
		var subNode = vNode.k;
		var tagger = vNode.j;

		while (subNode.$ === 4)
		{
			typeof tagger !== 'object'
				? tagger = [tagger, subNode.j]
				: tagger.push(subNode.j);

			subNode = subNode.k;
		}

		var subEventRoot = { j: tagger, p: eventNode };
		var domNode = _VirtualDom_render(subNode, subEventRoot);
		domNode.elm_event_node_ref = subEventRoot;
		return domNode;
	}

	if (tag === 3)
	{
		var domNode = vNode.h(vNode.g);
		_VirtualDom_applyFacts(domNode, eventNode, vNode.d);
		return domNode;
	}

	// at this point `tag` must be 1 or 2

	var domNode = vNode.f
		? _VirtualDom_doc.createElementNS(vNode.f, vNode.c)
		: _VirtualDom_doc.createElement(vNode.c);

	if (_VirtualDom_divertHrefToApp && vNode.c == 'a')
	{
		domNode.addEventListener('click', _VirtualDom_divertHrefToApp(domNode));
	}

	_VirtualDom_applyFacts(domNode, eventNode, vNode.d);

	for (var kids = vNode.e, i = 0; i < kids.length; i++)
	{
		_VirtualDom_appendChild(domNode, _VirtualDom_render(tag === 1 ? kids[i] : kids[i].b, eventNode));
	}

	return domNode;
}



// APPLY FACTS


function _VirtualDom_applyFacts(domNode, eventNode, facts)
{
	for (var key in facts)
	{
		var value = facts[key];

		key === 'a1'
			? _VirtualDom_applyStyles(domNode, value)
			:
		key === 'a0'
			? _VirtualDom_applyEvents(domNode, eventNode, value)
			:
		key === 'a3'
			? _VirtualDom_applyAttrs(domNode, value)
			:
		key === 'a4'
			? _VirtualDom_applyAttrsNS(domNode, value)
			:
		((key !== 'value' && key !== 'checked') || domNode[key] !== value) && (domNode[key] = value);
	}
}



// APPLY STYLES


function _VirtualDom_applyStyles(domNode, styles)
{
	var domNodeStyle = domNode.style;

	for (var key in styles)
	{
		domNodeStyle[key] = styles[key];
	}
}



// APPLY ATTRS


function _VirtualDom_applyAttrs(domNode, attrs)
{
	for (var key in attrs)
	{
		var value = attrs[key];
		typeof value !== 'undefined'
			? domNode.setAttribute(key, value)
			: domNode.removeAttribute(key);
	}
}



// APPLY NAMESPACED ATTRS


function _VirtualDom_applyAttrsNS(domNode, nsAttrs)
{
	for (var key in nsAttrs)
	{
		var pair = nsAttrs[key];
		var namespace = pair.f;
		var value = pair.o;

		typeof value !== 'undefined'
			? domNode.setAttributeNS(namespace, key, value)
			: domNode.removeAttributeNS(namespace, key);
	}
}



// APPLY EVENTS


function _VirtualDom_applyEvents(domNode, eventNode, events)
{
	var allCallbacks = domNode.elmFs || (domNode.elmFs = {});

	for (var key in events)
	{
		var newHandler = events[key];
		var oldCallback = allCallbacks[key];

		if (!newHandler)
		{
			domNode.removeEventListener(key, oldCallback);
			allCallbacks[key] = undefined;
			continue;
		}

		if (oldCallback)
		{
			var oldHandler = oldCallback.q;
			if (oldHandler.$ === newHandler.$)
			{
				oldCallback.q = newHandler;
				continue;
			}
			domNode.removeEventListener(key, oldCallback);
		}

		oldCallback = _VirtualDom_makeCallback(eventNode, newHandler);
		domNode.addEventListener(key, oldCallback,
			_VirtualDom_passiveSupported
			&& { passive: $elm$virtual_dom$VirtualDom$toHandlerInt(newHandler) < 2 }
		);
		allCallbacks[key] = oldCallback;
	}
}



// PASSIVE EVENTS


var _VirtualDom_passiveSupported;

try
{
	window.addEventListener('t', null, Object.defineProperty({}, 'passive', {
		get: function() { _VirtualDom_passiveSupported = true; }
	}));
}
catch(e) {}



// EVENT HANDLERS


function _VirtualDom_makeCallback(eventNode, initialHandler)
{
	function callback(event)
	{
		var handler = callback.q;
		var result = _Json_runHelp(handler.a, event);

		if (!$elm$core$Result$isOk(result))
		{
			return;
		}

		var tag = $elm$virtual_dom$VirtualDom$toHandlerInt(handler);

		// 0 = Normal
		// 1 = MayStopPropagation
		// 2 = MayPreventDefault
		// 3 = Custom

		var value = result.a;
		var message = !tag ? value : tag < 3 ? value.a : value.aa;
		var stopPropagation = tag == 1 ? value.b : tag == 3 && value.ba;
		var currentEventNode = (
			stopPropagation && event.stopPropagation(),
			(tag == 2 ? value.b : tag == 3 && value.a7) && event.preventDefault(),
			eventNode
		);
		var tagger;
		var i;
		while (tagger = currentEventNode.j)
		{
			if (typeof tagger == 'function')
			{
				message = tagger(message);
			}
			else
			{
				for (var i = tagger.length; i--; )
				{
					message = tagger[i](message);
				}
			}
			currentEventNode = currentEventNode.p;
		}
		currentEventNode(message, stopPropagation); // stopPropagation implies isSync
	}

	callback.q = initialHandler;

	return callback;
}

function _VirtualDom_equalEvents(x, y)
{
	return x.$ == y.$ && _Json_equality(x.a, y.a);
}



// DIFF


// TODO: Should we do patches like in iOS?
//
// type Patch
//   = At Int Patch
//   | Batch (List Patch)
//   | Change ...
//
// How could it not be better?
//
function _VirtualDom_diff(x, y)
{
	var patches = [];
	_VirtualDom_diffHelp(x, y, patches, 0);
	return patches;
}


function _VirtualDom_pushPatch(patches, type, index, data)
{
	var patch = {
		$: type,
		r: index,
		s: data,
		t: undefined,
		u: undefined
	};
	patches.push(patch);
	return patch;
}


function _VirtualDom_diffHelp(x, y, patches, index)
{
	if (x === y)
	{
		return;
	}

	var xType = x.$;
	var yType = y.$;

	// Bail if you run into different types of nodes. Implies that the
	// structure has changed significantly and it's not worth a diff.
	if (xType !== yType)
	{
		if (xType === 1 && yType === 2)
		{
			y = _VirtualDom_dekey(y);
			yType = 1;
		}
		else
		{
			_VirtualDom_pushPatch(patches, 0, index, y);
			return;
		}
	}

	// Now we know that both nodes are the same $.
	switch (yType)
	{
		case 5:
			var xRefs = x.l;
			var yRefs = y.l;
			var i = xRefs.length;
			var same = i === yRefs.length;
			while (same && i--)
			{
				same = xRefs[i] === yRefs[i];
			}
			if (same)
			{
				y.k = x.k;
				return;
			}
			y.k = y.m();
			var subPatches = [];
			_VirtualDom_diffHelp(x.k, y.k, subPatches, 0);
			subPatches.length > 0 && _VirtualDom_pushPatch(patches, 1, index, subPatches);
			return;

		case 4:
			// gather nested taggers
			var xTaggers = x.j;
			var yTaggers = y.j;
			var nesting = false;

			var xSubNode = x.k;
			while (xSubNode.$ === 4)
			{
				nesting = true;

				typeof xTaggers !== 'object'
					? xTaggers = [xTaggers, xSubNode.j]
					: xTaggers.push(xSubNode.j);

				xSubNode = xSubNode.k;
			}

			var ySubNode = y.k;
			while (ySubNode.$ === 4)
			{
				nesting = true;

				typeof yTaggers !== 'object'
					? yTaggers = [yTaggers, ySubNode.j]
					: yTaggers.push(ySubNode.j);

				ySubNode = ySubNode.k;
			}

			// Just bail if different numbers of taggers. This implies the
			// structure of the virtual DOM has changed.
			if (nesting && xTaggers.length !== yTaggers.length)
			{
				_VirtualDom_pushPatch(patches, 0, index, y);
				return;
			}

			// check if taggers are "the same"
			if (nesting ? !_VirtualDom_pairwiseRefEqual(xTaggers, yTaggers) : xTaggers !== yTaggers)
			{
				_VirtualDom_pushPatch(patches, 2, index, yTaggers);
			}

			// diff everything below the taggers
			_VirtualDom_diffHelp(xSubNode, ySubNode, patches, index + 1);
			return;

		case 0:
			if (x.a !== y.a)
			{
				_VirtualDom_pushPatch(patches, 3, index, y.a);
			}
			return;

		case 1:
			_VirtualDom_diffNodes(x, y, patches, index, _VirtualDom_diffKids);
			return;

		case 2:
			_VirtualDom_diffNodes(x, y, patches, index, _VirtualDom_diffKeyedKids);
			return;

		case 3:
			if (x.h !== y.h)
			{
				_VirtualDom_pushPatch(patches, 0, index, y);
				return;
			}

			var factsDiff = _VirtualDom_diffFacts(x.d, y.d);
			factsDiff && _VirtualDom_pushPatch(patches, 4, index, factsDiff);

			var patch = y.i(x.g, y.g);
			patch && _VirtualDom_pushPatch(patches, 5, index, patch);

			return;
	}
}

// assumes the incoming arrays are the same length
function _VirtualDom_pairwiseRefEqual(as, bs)
{
	for (var i = 0; i < as.length; i++)
	{
		if (as[i] !== bs[i])
		{
			return false;
		}
	}

	return true;
}

function _VirtualDom_diffNodes(x, y, patches, index, diffKids)
{
	// Bail if obvious indicators have changed. Implies more serious
	// structural changes such that it's not worth it to diff.
	if (x.c !== y.c || x.f !== y.f)
	{
		_VirtualDom_pushPatch(patches, 0, index, y);
		return;
	}

	var factsDiff = _VirtualDom_diffFacts(x.d, y.d);
	factsDiff && _VirtualDom_pushPatch(patches, 4, index, factsDiff);

	diffKids(x, y, patches, index);
}



// DIFF FACTS


// TODO Instead of creating a new diff object, it's possible to just test if
// there *is* a diff. During the actual patch, do the diff again and make the
// modifications directly. This way, there's no new allocations. Worth it?
function _VirtualDom_diffFacts(x, y, category)
{
	var diff;

	// look for changes and removals
	for (var xKey in x)
	{
		if (xKey === 'a1' || xKey === 'a0' || xKey === 'a3' || xKey === 'a4')
		{
			var subDiff = _VirtualDom_diffFacts(x[xKey], y[xKey] || {}, xKey);
			if (subDiff)
			{
				diff = diff || {};
				diff[xKey] = subDiff;
			}
			continue;
		}

		// remove if not in the new facts
		if (!(xKey in y))
		{
			diff = diff || {};
			diff[xKey] =
				!category
					? (typeof x[xKey] === 'string' ? '' : null)
					:
				(category === 'a1')
					? ''
					:
				(category === 'a0' || category === 'a3')
					? undefined
					:
				{ f: x[xKey].f, o: undefined };

			continue;
		}

		var xValue = x[xKey];
		var yValue = y[xKey];

		// reference equal, so don't worry about it
		if (xValue === yValue && xKey !== 'value' && xKey !== 'checked'
			|| category === 'a0' && _VirtualDom_equalEvents(xValue, yValue))
		{
			continue;
		}

		diff = diff || {};
		diff[xKey] = yValue;
	}

	// add new stuff
	for (var yKey in y)
	{
		if (!(yKey in x))
		{
			diff = diff || {};
			diff[yKey] = y[yKey];
		}
	}

	return diff;
}



// DIFF KIDS


function _VirtualDom_diffKids(xParent, yParent, patches, index)
{
	var xKids = xParent.e;
	var yKids = yParent.e;

	var xLen = xKids.length;
	var yLen = yKids.length;

	// FIGURE OUT IF THERE ARE INSERTS OR REMOVALS

	if (xLen > yLen)
	{
		_VirtualDom_pushPatch(patches, 6, index, {
			v: yLen,
			i: xLen - yLen
		});
	}
	else if (xLen < yLen)
	{
		_VirtualDom_pushPatch(patches, 7, index, {
			v: xLen,
			e: yKids
		});
	}

	// PAIRWISE DIFF EVERYTHING ELSE

	for (var minLen = xLen < yLen ? xLen : yLen, i = 0; i < minLen; i++)
	{
		var xKid = xKids[i];
		_VirtualDom_diffHelp(xKid, yKids[i], patches, ++index);
		index += xKid.b || 0;
	}
}



// KEYED DIFF


function _VirtualDom_diffKeyedKids(xParent, yParent, patches, rootIndex)
{
	var localPatches = [];

	var changes = {}; // Dict String Entry
	var inserts = []; // Array { index : Int, entry : Entry }
	// type Entry = { tag : String, vnode : VNode, index : Int, data : _ }

	var xKids = xParent.e;
	var yKids = yParent.e;
	var xLen = xKids.length;
	var yLen = yKids.length;
	var xIndex = 0;
	var yIndex = 0;

	var index = rootIndex;

	while (xIndex < xLen && yIndex < yLen)
	{
		var x = xKids[xIndex];
		var y = yKids[yIndex];

		var xKey = x.a;
		var yKey = y.a;
		var xNode = x.b;
		var yNode = y.b;

		var newMatch = undefined;
		var oldMatch = undefined;

		// check if keys match

		if (xKey === yKey)
		{
			index++;
			_VirtualDom_diffHelp(xNode, yNode, localPatches, index);
			index += xNode.b || 0;

			xIndex++;
			yIndex++;
			continue;
		}

		// look ahead 1 to detect insertions and removals.

		var xNext = xKids[xIndex + 1];
		var yNext = yKids[yIndex + 1];

		if (xNext)
		{
			var xNextKey = xNext.a;
			var xNextNode = xNext.b;
			oldMatch = yKey === xNextKey;
		}

		if (yNext)
		{
			var yNextKey = yNext.a;
			var yNextNode = yNext.b;
			newMatch = xKey === yNextKey;
		}


		// swap x and y
		if (newMatch && oldMatch)
		{
			index++;
			_VirtualDom_diffHelp(xNode, yNextNode, localPatches, index);
			_VirtualDom_insertNode(changes, localPatches, xKey, yNode, yIndex, inserts);
			index += xNode.b || 0;

			index++;
			_VirtualDom_removeNode(changes, localPatches, xKey, xNextNode, index);
			index += xNextNode.b || 0;

			xIndex += 2;
			yIndex += 2;
			continue;
		}

		// insert y
		if (newMatch)
		{
			index++;
			_VirtualDom_insertNode(changes, localPatches, yKey, yNode, yIndex, inserts);
			_VirtualDom_diffHelp(xNode, yNextNode, localPatches, index);
			index += xNode.b || 0;

			xIndex += 1;
			yIndex += 2;
			continue;
		}

		// remove x
		if (oldMatch)
		{
			index++;
			_VirtualDom_removeNode(changes, localPatches, xKey, xNode, index);
			index += xNode.b || 0;

			index++;
			_VirtualDom_diffHelp(xNextNode, yNode, localPatches, index);
			index += xNextNode.b || 0;

			xIndex += 2;
			yIndex += 1;
			continue;
		}

		// remove x, insert y
		if (xNext && xNextKey === yNextKey)
		{
			index++;
			_VirtualDom_removeNode(changes, localPatches, xKey, xNode, index);
			_VirtualDom_insertNode(changes, localPatches, yKey, yNode, yIndex, inserts);
			index += xNode.b || 0;

			index++;
			_VirtualDom_diffHelp(xNextNode, yNextNode, localPatches, index);
			index += xNextNode.b || 0;

			xIndex += 2;
			yIndex += 2;
			continue;
		}

		break;
	}

	// eat up any remaining nodes with removeNode and insertNode

	while (xIndex < xLen)
	{
		index++;
		var x = xKids[xIndex];
		var xNode = x.b;
		_VirtualDom_removeNode(changes, localPatches, x.a, xNode, index);
		index += xNode.b || 0;
		xIndex++;
	}

	while (yIndex < yLen)
	{
		var endInserts = endInserts || [];
		var y = yKids[yIndex];
		_VirtualDom_insertNode(changes, localPatches, y.a, y.b, undefined, endInserts);
		yIndex++;
	}

	if (localPatches.length > 0 || inserts.length > 0 || endInserts)
	{
		_VirtualDom_pushPatch(patches, 8, rootIndex, {
			w: localPatches,
			x: inserts,
			y: endInserts
		});
	}
}



// CHANGES FROM KEYED DIFF


var _VirtualDom_POSTFIX = '_elmW6BL';


function _VirtualDom_insertNode(changes, localPatches, key, vnode, yIndex, inserts)
{
	var entry = changes[key];

	// never seen this key before
	if (!entry)
	{
		entry = {
			c: 0,
			z: vnode,
			r: yIndex,
			s: undefined
		};

		inserts.push({ r: yIndex, A: entry });
		changes[key] = entry;

		return;
	}

	// this key was removed earlier, a match!
	if (entry.c === 1)
	{
		inserts.push({ r: yIndex, A: entry });

		entry.c = 2;
		var subPatches = [];
		_VirtualDom_diffHelp(entry.z, vnode, subPatches, entry.r);
		entry.r = yIndex;
		entry.s.s = {
			w: subPatches,
			A: entry
		};

		return;
	}

	// this key has already been inserted or moved, a duplicate!
	_VirtualDom_insertNode(changes, localPatches, key + _VirtualDom_POSTFIX, vnode, yIndex, inserts);
}


function _VirtualDom_removeNode(changes, localPatches, key, vnode, index)
{
	var entry = changes[key];

	// never seen this key before
	if (!entry)
	{
		var patch = _VirtualDom_pushPatch(localPatches, 9, index, undefined);

		changes[key] = {
			c: 1,
			z: vnode,
			r: index,
			s: patch
		};

		return;
	}

	// this key was inserted earlier, a match!
	if (entry.c === 0)
	{
		entry.c = 2;
		var subPatches = [];
		_VirtualDom_diffHelp(vnode, entry.z, subPatches, index);

		_VirtualDom_pushPatch(localPatches, 9, index, {
			w: subPatches,
			A: entry
		});

		return;
	}

	// this key has already been removed or moved, a duplicate!
	_VirtualDom_removeNode(changes, localPatches, key + _VirtualDom_POSTFIX, vnode, index);
}



// ADD DOM NODES
//
// Each DOM node has an "index" assigned in order of traversal. It is important
// to minimize our crawl over the actual DOM, so these indexes (along with the
// descendantsCount of virtual nodes) let us skip touching entire subtrees of
// the DOM if we know there are no patches there.


function _VirtualDom_addDomNodes(domNode, vNode, patches, eventNode)
{
	_VirtualDom_addDomNodesHelp(domNode, vNode, patches, 0, 0, vNode.b, eventNode);
}


// assumes `patches` is non-empty and indexes increase monotonically.
function _VirtualDom_addDomNodesHelp(domNode, vNode, patches, i, low, high, eventNode)
{
	var patch = patches[i];
	var index = patch.r;

	while (index === low)
	{
		var patchType = patch.$;

		if (patchType === 1)
		{
			_VirtualDom_addDomNodes(domNode, vNode.k, patch.s, eventNode);
		}
		else if (patchType === 8)
		{
			patch.t = domNode;
			patch.u = eventNode;

			var subPatches = patch.s.w;
			if (subPatches.length > 0)
			{
				_VirtualDom_addDomNodesHelp(domNode, vNode, subPatches, 0, low, high, eventNode);
			}
		}
		else if (patchType === 9)
		{
			patch.t = domNode;
			patch.u = eventNode;

			var data = patch.s;
			if (data)
			{
				data.A.s = domNode;
				var subPatches = data.w;
				if (subPatches.length > 0)
				{
					_VirtualDom_addDomNodesHelp(domNode, vNode, subPatches, 0, low, high, eventNode);
				}
			}
		}
		else
		{
			patch.t = domNode;
			patch.u = eventNode;
		}

		i++;

		if (!(patch = patches[i]) || (index = patch.r) > high)
		{
			return i;
		}
	}

	var tag = vNode.$;

	if (tag === 4)
	{
		var subNode = vNode.k;

		while (subNode.$ === 4)
		{
			subNode = subNode.k;
		}

		return _VirtualDom_addDomNodesHelp(domNode, subNode, patches, i, low + 1, high, domNode.elm_event_node_ref);
	}

	// tag must be 1 or 2 at this point

	var vKids = vNode.e;
	var childNodes = domNode.childNodes;
	for (var j = 0; j < vKids.length; j++)
	{
		low++;
		var vKid = tag === 1 ? vKids[j] : vKids[j].b;
		var nextLow = low + (vKid.b || 0);
		if (low <= index && index <= nextLow)
		{
			i = _VirtualDom_addDomNodesHelp(childNodes[j], vKid, patches, i, low, nextLow, eventNode);
			if (!(patch = patches[i]) || (index = patch.r) > high)
			{
				return i;
			}
		}
		low = nextLow;
	}
	return i;
}



// APPLY PATCHES


function _VirtualDom_applyPatches(rootDomNode, oldVirtualNode, patches, eventNode)
{
	if (patches.length === 0)
	{
		return rootDomNode;
	}

	_VirtualDom_addDomNodes(rootDomNode, oldVirtualNode, patches, eventNode);
	return _VirtualDom_applyPatchesHelp(rootDomNode, patches);
}

function _VirtualDom_applyPatchesHelp(rootDomNode, patches)
{
	for (var i = 0; i < patches.length; i++)
	{
		var patch = patches[i];
		var localDomNode = patch.t
		var newNode = _VirtualDom_applyPatch(localDomNode, patch);
		if (localDomNode === rootDomNode)
		{
			rootDomNode = newNode;
		}
	}
	return rootDomNode;
}

function _VirtualDom_applyPatch(domNode, patch)
{
	switch (patch.$)
	{
		case 0:
			return _VirtualDom_applyPatchRedraw(domNode, patch.s, patch.u);

		case 4:
			_VirtualDom_applyFacts(domNode, patch.u, patch.s);
			return domNode;

		case 3:
			domNode.replaceData(0, domNode.length, patch.s);
			return domNode;

		case 1:
			return _VirtualDom_applyPatchesHelp(domNode, patch.s);

		case 2:
			if (domNode.elm_event_node_ref)
			{
				domNode.elm_event_node_ref.j = patch.s;
			}
			else
			{
				domNode.elm_event_node_ref = { j: patch.s, p: patch.u };
			}
			return domNode;

		case 6:
			var data = patch.s;
			for (var i = 0; i < data.i; i++)
			{
				domNode.removeChild(domNode.childNodes[data.v]);
			}
			return domNode;

		case 7:
			var data = patch.s;
			var kids = data.e;
			var i = data.v;
			var theEnd = domNode.childNodes[i];
			for (; i < kids.length; i++)
			{
				domNode.insertBefore(_VirtualDom_render(kids[i], patch.u), theEnd);
			}
			return domNode;

		case 9:
			var data = patch.s;
			if (!data)
			{
				domNode.parentNode.removeChild(domNode);
				return domNode;
			}
			var entry = data.A;
			if (typeof entry.r !== 'undefined')
			{
				domNode.parentNode.removeChild(domNode);
			}
			entry.s = _VirtualDom_applyPatchesHelp(domNode, data.w);
			return domNode;

		case 8:
			return _VirtualDom_applyPatchReorder(domNode, patch);

		case 5:
			return patch.s(domNode);

		default:
			_Debug_crash(10); // 'Ran into an unknown patch!'
	}
}


function _VirtualDom_applyPatchRedraw(domNode, vNode, eventNode)
{
	var parentNode = domNode.parentNode;
	var newNode = _VirtualDom_render(vNode, eventNode);

	if (!newNode.elm_event_node_ref)
	{
		newNode.elm_event_node_ref = domNode.elm_event_node_ref;
	}

	if (parentNode && newNode !== domNode)
	{
		parentNode.replaceChild(newNode, domNode);
	}
	return newNode;
}


function _VirtualDom_applyPatchReorder(domNode, patch)
{
	var data = patch.s;

	// remove end inserts
	var frag = _VirtualDom_applyPatchReorderEndInsertsHelp(data.y, patch);

	// removals
	domNode = _VirtualDom_applyPatchesHelp(domNode, data.w);

	// inserts
	var inserts = data.x;
	for (var i = 0; i < inserts.length; i++)
	{
		var insert = inserts[i];
		var entry = insert.A;
		var node = entry.c === 2
			? entry.s
			: _VirtualDom_render(entry.z, patch.u);
		domNode.insertBefore(node, domNode.childNodes[insert.r]);
	}

	// add end inserts
	if (frag)
	{
		_VirtualDom_appendChild(domNode, frag);
	}

	return domNode;
}


function _VirtualDom_applyPatchReorderEndInsertsHelp(endInserts, patch)
{
	if (!endInserts)
	{
		return;
	}

	var frag = _VirtualDom_doc.createDocumentFragment();
	for (var i = 0; i < endInserts.length; i++)
	{
		var insert = endInserts[i];
		var entry = insert.A;
		_VirtualDom_appendChild(frag, entry.c === 2
			? entry.s
			: _VirtualDom_render(entry.z, patch.u)
		);
	}
	return frag;
}


function _VirtualDom_virtualize(node)
{
	// TEXT NODES

	if (node.nodeType === 3)
	{
		return _VirtualDom_text(node.textContent);
	}


	// WEIRD NODES

	if (node.nodeType !== 1)
	{
		return _VirtualDom_text('');
	}


	// ELEMENT NODES

	var attrList = _List_Nil;
	var attrs = node.attributes;
	for (var i = attrs.length; i--; )
	{
		var attr = attrs[i];
		var name = attr.name;
		var value = attr.value;
		attrList = _List_Cons( A2(_VirtualDom_attribute, name, value), attrList );
	}

	var tag = node.tagName.toLowerCase();
	var kidList = _List_Nil;
	var kids = node.childNodes;

	for (var i = kids.length; i--; )
	{
		kidList = _List_Cons(_VirtualDom_virtualize(kids[i]), kidList);
	}
	return A3(_VirtualDom_node, tag, attrList, kidList);
}

function _VirtualDom_dekey(keyedNode)
{
	var keyedKids = keyedNode.e;
	var len = keyedKids.length;
	var kids = new Array(len);
	for (var i = 0; i < len; i++)
	{
		kids[i] = keyedKids[i].b;
	}

	return {
		$: 1,
		c: keyedNode.c,
		d: keyedNode.d,
		e: kids,
		f: keyedNode.f,
		b: keyedNode.b
	};
}




// ELEMENT


var _Debugger_element;

var _Browser_element = _Debugger_element || F4(function(impl, flagDecoder, debugMetadata, args)
{
	return _Platform_initialize(
		flagDecoder,
		args,
		impl.bU,
		impl.b5,
		impl.b3,
		function(sendToApp, initialModel) {
			var view = impl.b6;
			/**/
			var domNode = args['node'];
			//*/
			/**_UNUSED/
			var domNode = args && args['node'] ? args['node'] : _Debug_crash(0);
			//*/
			var currNode = _VirtualDom_virtualize(domNode);

			return _Browser_makeAnimator(initialModel, function(model)
			{
				var nextNode = view(model);
				var patches = _VirtualDom_diff(currNode, nextNode);
				domNode = _VirtualDom_applyPatches(domNode, currNode, patches, sendToApp);
				currNode = nextNode;
			});
		}
	);
});



// DOCUMENT


var _Debugger_document;

var _Browser_document = _Debugger_document || F4(function(impl, flagDecoder, debugMetadata, args)
{
	return _Platform_initialize(
		flagDecoder,
		args,
		impl.bU,
		impl.b5,
		impl.b3,
		function(sendToApp, initialModel) {
			var divertHrefToApp = impl.a8 && impl.a8(sendToApp)
			var view = impl.b6;
			var title = _VirtualDom_doc.title;
			var bodyNode = _VirtualDom_doc.body;
			var currNode = _VirtualDom_virtualize(bodyNode);
			return _Browser_makeAnimator(initialModel, function(model)
			{
				_VirtualDom_divertHrefToApp = divertHrefToApp;
				var doc = view(model);
				var nextNode = _VirtualDom_node('body')(_List_Nil)(doc.aG);
				var patches = _VirtualDom_diff(currNode, nextNode);
				bodyNode = _VirtualDom_applyPatches(bodyNode, currNode, patches, sendToApp);
				currNode = nextNode;
				_VirtualDom_divertHrefToApp = 0;
				(title !== doc.b4) && (_VirtualDom_doc.title = title = doc.b4);
			});
		}
	);
});



// ANIMATION


var _Browser_cancelAnimationFrame =
	typeof cancelAnimationFrame !== 'undefined'
		? cancelAnimationFrame
		: function(id) { clearTimeout(id); };

var _Browser_requestAnimationFrame =
	typeof requestAnimationFrame !== 'undefined'
		? requestAnimationFrame
		: function(callback) { return setTimeout(callback, 1000 / 60); };


function _Browser_makeAnimator(model, draw)
{
	draw(model);

	var state = 0;

	function updateIfNeeded()
	{
		state = state === 1
			? 0
			: ( _Browser_requestAnimationFrame(updateIfNeeded), draw(model), 1 );
	}

	return function(nextModel, isSync)
	{
		model = nextModel;

		isSync
			? ( draw(model),
				state === 2 && (state = 1)
				)
			: ( state === 0 && _Browser_requestAnimationFrame(updateIfNeeded),
				state = 2
				);
	};
}



// APPLICATION


function _Browser_application(impl)
{
	var onUrlChange = impl.bW;
	var onUrlRequest = impl.bX;
	var key = function() { key.a(onUrlChange(_Browser_getUrl())); };

	return _Browser_document({
		a8: function(sendToApp)
		{
			key.a = sendToApp;
			_Browser_window.addEventListener('popstate', key);
			_Browser_window.navigator.userAgent.indexOf('Trident') < 0 || _Browser_window.addEventListener('hashchange', key);

			return F2(function(domNode, event)
			{
				if (!event.ctrlKey && !event.metaKey && !event.shiftKey && event.button < 1 && !domNode.target && !domNode.hasAttribute('download'))
				{
					event.preventDefault();
					var href = domNode.href;
					var curr = _Browser_getUrl();
					var next = $elm$url$Url$fromString(href).a;
					sendToApp(onUrlRequest(
						(next
							&& curr.bu === next.bu
							&& curr.bj === next.bj
							&& curr.br.a === next.br.a
						)
							? $elm$browser$Browser$Internal(next)
							: $elm$browser$Browser$External(href)
					));
				}
			});
		},
		bU: function(flags)
		{
			return A3(impl.bU, flags, _Browser_getUrl(), key);
		},
		b6: impl.b6,
		b5: impl.b5,
		b3: impl.b3
	});
}

function _Browser_getUrl()
{
	return $elm$url$Url$fromString(_VirtualDom_doc.location.href).a || _Debug_crash(1);
}

var _Browser_go = F2(function(key, n)
{
	return A2($elm$core$Task$perform, $elm$core$Basics$never, _Scheduler_binding(function() {
		n && history.go(n);
		key();
	}));
});

var _Browser_pushUrl = F2(function(key, url)
{
	return A2($elm$core$Task$perform, $elm$core$Basics$never, _Scheduler_binding(function() {
		history.pushState({}, '', url);
		key();
	}));
});

var _Browser_replaceUrl = F2(function(key, url)
{
	return A2($elm$core$Task$perform, $elm$core$Basics$never, _Scheduler_binding(function() {
		history.replaceState({}, '', url);
		key();
	}));
});



// GLOBAL EVENTS


var _Browser_fakeNode = { addEventListener: function() {}, removeEventListener: function() {} };
var _Browser_doc = typeof document !== 'undefined' ? document : _Browser_fakeNode;
var _Browser_window = typeof window !== 'undefined' ? window : _Browser_fakeNode;

var _Browser_on = F3(function(node, eventName, sendToSelf)
{
	return _Scheduler_spawn(_Scheduler_binding(function(callback)
	{
		function handler(event)	{ _Scheduler_rawSpawn(sendToSelf(event)); }
		node.addEventListener(eventName, handler, _VirtualDom_passiveSupported && { passive: true });
		return function() { node.removeEventListener(eventName, handler); };
	}));
});

var _Browser_decodeEvent = F2(function(decoder, event)
{
	var result = _Json_runHelp(decoder, event);
	return $elm$core$Result$isOk(result) ? $elm$core$Maybe$Just(result.a) : $elm$core$Maybe$Nothing;
});



// PAGE VISIBILITY


function _Browser_visibilityInfo()
{
	return (typeof _VirtualDom_doc.hidden !== 'undefined')
		? { bS: 'hidden', bN: 'visibilitychange' }
		:
	(typeof _VirtualDom_doc.mozHidden !== 'undefined')
		? { bS: 'mozHidden', bN: 'mozvisibilitychange' }
		:
	(typeof _VirtualDom_doc.msHidden !== 'undefined')
		? { bS: 'msHidden', bN: 'msvisibilitychange' }
		:
	(typeof _VirtualDom_doc.webkitHidden !== 'undefined')
		? { bS: 'webkitHidden', bN: 'webkitvisibilitychange' }
		: { bS: 'hidden', bN: 'visibilitychange' };
}



// ANIMATION FRAMES


function _Browser_rAF()
{
	return _Scheduler_binding(function(callback)
	{
		var id = _Browser_requestAnimationFrame(function() {
			callback(_Scheduler_succeed(Date.now()));
		});

		return function() {
			_Browser_cancelAnimationFrame(id);
		};
	});
}


function _Browser_now()
{
	return _Scheduler_binding(function(callback)
	{
		callback(_Scheduler_succeed(Date.now()));
	});
}



// DOM STUFF


function _Browser_withNode(id, doStuff)
{
	return _Scheduler_binding(function(callback)
	{
		_Browser_requestAnimationFrame(function() {
			var node = document.getElementById(id);
			callback(node
				? _Scheduler_succeed(doStuff(node))
				: _Scheduler_fail($elm$browser$Browser$Dom$NotFound(id))
			);
		});
	});
}


function _Browser_withWindow(doStuff)
{
	return _Scheduler_binding(function(callback)
	{
		_Browser_requestAnimationFrame(function() {
			callback(_Scheduler_succeed(doStuff()));
		});
	});
}


// FOCUS and BLUR


var _Browser_call = F2(function(functionName, id)
{
	return _Browser_withNode(id, function(node) {
		node[functionName]();
		return _Utils_Tuple0;
	});
});



// WINDOW VIEWPORT


function _Browser_getViewport()
{
	return {
		bA: _Browser_getScene(),
		bI: {
			h: _Browser_window.pageXOffset,
			i: _Browser_window.pageYOffset,
			l: _Browser_doc.documentElement.clientWidth,
			j: _Browser_doc.documentElement.clientHeight
		}
	};
}

function _Browser_getScene()
{
	var body = _Browser_doc.body;
	var elem = _Browser_doc.documentElement;
	return {
		l: Math.max(body.scrollWidth, body.offsetWidth, elem.scrollWidth, elem.offsetWidth, elem.clientWidth),
		j: Math.max(body.scrollHeight, body.offsetHeight, elem.scrollHeight, elem.offsetHeight, elem.clientHeight)
	};
}

var _Browser_setViewport = F2(function(x, y)
{
	return _Browser_withWindow(function()
	{
		_Browser_window.scroll(x, y);
		return _Utils_Tuple0;
	});
});



// ELEMENT VIEWPORT


function _Browser_getViewportOf(id)
{
	return _Browser_withNode(id, function(node)
	{
		return {
			bA: {
				l: node.scrollWidth,
				j: node.scrollHeight
			},
			bI: {
				h: node.scrollLeft,
				i: node.scrollTop,
				l: node.clientWidth,
				j: node.clientHeight
			}
		};
	});
}


var _Browser_setViewportOf = F3(function(id, x, y)
{
	return _Browser_withNode(id, function(node)
	{
		node.scrollLeft = x;
		node.scrollTop = y;
		return _Utils_Tuple0;
	});
});



// ELEMENT


function _Browser_getElement(id)
{
	return _Browser_withNode(id, function(node)
	{
		var rect = node.getBoundingClientRect();
		var x = _Browser_window.pageXOffset;
		var y = _Browser_window.pageYOffset;
		return {
			bA: _Browser_getScene(),
			bI: {
				h: x,
				i: y,
				l: _Browser_doc.documentElement.clientWidth,
				j: _Browser_doc.documentElement.clientHeight
			},
			bP: {
				h: x + rect.left,
				i: y + rect.top,
				l: rect.width,
				j: rect.height
			}
		};
	});
}



// LOAD and RELOAD


function _Browser_reload(skipCache)
{
	return A2($elm$core$Task$perform, $elm$core$Basics$never, _Scheduler_binding(function(callback)
	{
		_VirtualDom_doc.location.reload(skipCache);
	}));
}

function _Browser_load(url)
{
	return A2($elm$core$Task$perform, $elm$core$Basics$never, _Scheduler_binding(function(callback)
	{
		try
		{
			_Browser_window.location = url;
		}
		catch(err)
		{
			// Only Firefox can throw a NS_ERROR_MALFORMED_URI exception here.
			// Other browsers reload the page, so let's be consistent about that.
			_VirtualDom_doc.location.reload(false);
		}
	}));
}



// SEND REQUEST

var _Http_toTask = F3(function(router, toTask, request)
{
	return _Scheduler_binding(function(callback)
	{
		function done(response) {
			callback(toTask(request.as.a(response)));
		}

		var xhr = new XMLHttpRequest();
		xhr.addEventListener('error', function() { done($elm$http$Http$NetworkError_); });
		xhr.addEventListener('timeout', function() { done($elm$http$Http$Timeout_); });
		xhr.addEventListener('load', function() { done(_Http_toResponse(request.as.b, xhr)); });
		$elm$core$Maybe$isJust(request.bG) && _Http_track(router, xhr, request.bG.a);

		try {
			xhr.open(request.bl, request.aA, true);
		} catch (e) {
			return done($elm$http$Http$BadUrl_(request.aA));
		}

		_Http_configureRequest(xhr, request);

		request.aG.a && xhr.setRequestHeader('Content-Type', request.aG.a);
		xhr.send(request.aG.b);

		return function() { xhr.c = true; xhr.abort(); };
	});
});


// CONFIGURE

function _Http_configureRequest(xhr, request)
{
	for (var headers = request.bi; headers.b; headers = headers.b) // WHILE_CONS
	{
		xhr.setRequestHeader(headers.a.a, headers.a.b);
	}
	xhr.timeout = request.bF.a || 0;
	xhr.responseType = request.as.d;
	xhr.withCredentials = request.bK;
}


// RESPONSES

function _Http_toResponse(toBody, xhr)
{
	return A2(
		200 <= xhr.status && xhr.status < 300 ? $elm$http$Http$GoodStatus_ : $elm$http$Http$BadStatus_,
		_Http_toMetadata(xhr),
		toBody(xhr.response)
	);
}


// METADATA

function _Http_toMetadata(xhr)
{
	return {
		aA: xhr.responseURL,
		b1: xhr.status,
		b2: xhr.statusText,
		bi: _Http_parseHeaders(xhr.getAllResponseHeaders())
	};
}


// HEADERS

function _Http_parseHeaders(rawHeaders)
{
	if (!rawHeaders)
	{
		return $elm$core$Dict$empty;
	}

	var headers = $elm$core$Dict$empty;
	var headerPairs = rawHeaders.split('\r\n');
	for (var i = headerPairs.length; i--; )
	{
		var headerPair = headerPairs[i];
		var index = headerPair.indexOf(': ');
		if (index > 0)
		{
			var key = headerPair.substring(0, index);
			var value = headerPair.substring(index + 2);

			headers = A3($elm$core$Dict$update, key, function(oldValue) {
				return $elm$core$Maybe$Just($elm$core$Maybe$isJust(oldValue)
					? value + ', ' + oldValue.a
					: value
				);
			}, headers);
		}
	}
	return headers;
}


// EXPECT

var _Http_expect = F3(function(type, toBody, toValue)
{
	return {
		$: 0,
		d: type,
		b: toBody,
		a: toValue
	};
});

var _Http_mapExpect = F2(function(func, expect)
{
	return {
		$: 0,
		d: expect.d,
		b: expect.b,
		a: function(x) { return func(expect.a(x)); }
	};
});

function _Http_toDataView(arrayBuffer)
{
	return new DataView(arrayBuffer);
}


// BODY and PARTS

var _Http_emptyBody = { $: 0 };
var _Http_pair = F2(function(a, b) { return { $: 0, a: a, b: b }; });

function _Http_toFormData(parts)
{
	for (var formData = new FormData(); parts.b; parts = parts.b) // WHILE_CONS
	{
		var part = parts.a;
		formData.append(part.a, part.b);
	}
	return formData;
}

var _Http_bytesToBlob = F2(function(mime, bytes)
{
	return new Blob([bytes], { type: mime });
});


// PROGRESS

function _Http_track(router, xhr, tracker)
{
	// TODO check out lengthComputable on loadstart event

	xhr.upload.addEventListener('progress', function(event) {
		if (xhr.c) { return; }
		_Scheduler_rawSpawn(A2($elm$core$Platform$sendToSelf, router, _Utils_Tuple2(tracker, $elm$http$Http$Sending({
			b0: event.loaded,
			bB: event.total
		}))));
	});
	xhr.addEventListener('progress', function(event) {
		if (xhr.c) { return; }
		_Scheduler_rawSpawn(A2($elm$core$Platform$sendToSelf, router, _Utils_Tuple2(tracker, $elm$http$Http$Receiving({
			bZ: event.loaded,
			bB: event.lengthComputable ? $elm$core$Maybe$Just(event.total) : $elm$core$Maybe$Nothing
		}))));
	});
}


// DECODER

var _File_decoder = _Json_decodePrim(function(value) {
	// NOTE: checks if `File` exists in case this is run on node
	return (typeof File !== 'undefined' && value instanceof File)
		? $elm$core$Result$Ok(value)
		: _Json_expecting('a FILE', value);
});


// METADATA

function _File_name(file) { return file.name; }
function _File_mime(file) { return file.type; }
function _File_size(file) { return file.size; }

function _File_lastModified(file)
{
	return $elm$time$Time$millisToPosix(file.lastModified);
}


// DOWNLOAD

var _File_downloadNode;

function _File_getDownloadNode()
{
	return _File_downloadNode || (_File_downloadNode = document.createElement('a'));
}

var _File_download = F3(function(name, mime, content)
{
	return _Scheduler_binding(function(callback)
	{
		var blob = new Blob([content], {type: mime});

		// for IE10+
		if (navigator.msSaveOrOpenBlob)
		{
			navigator.msSaveOrOpenBlob(blob, name);
			return;
		}

		// for HTML5
		var node = _File_getDownloadNode();
		var objectUrl = URL.createObjectURL(blob);
		node.href = objectUrl;
		node.download = name;
		_File_click(node);
		URL.revokeObjectURL(objectUrl);
	});
});

function _File_downloadUrl(href)
{
	return _Scheduler_binding(function(callback)
	{
		var node = _File_getDownloadNode();
		node.href = href;
		node.download = '';
		node.origin === location.origin || (node.target = '_blank');
		_File_click(node);
	});
}


// IE COMPATIBILITY

function _File_makeBytesSafeForInternetExplorer(bytes)
{
	// only needed by IE10 and IE11 to fix https://github.com/elm/file/issues/10
	// all other browsers can just run `new Blob([bytes])` directly with no problem
	//
	return new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength);
}

function _File_click(node)
{
	// only needed by IE10 and IE11 to fix https://github.com/elm/file/issues/11
	// all other browsers have MouseEvent and do not need this conditional stuff
	//
	if (typeof MouseEvent === 'function')
	{
		node.dispatchEvent(new MouseEvent('click'));
	}
	else
	{
		var event = document.createEvent('MouseEvents');
		event.initMouseEvent('click', true, true, window, 0, 0, 0, 0, 0, false, false, false, false, 0, null);
		document.body.appendChild(node);
		node.dispatchEvent(event);
		document.body.removeChild(node);
	}
}


// UPLOAD

var _File_node;

function _File_uploadOne(mimes)
{
	return _Scheduler_binding(function(callback)
	{
		_File_node = document.createElement('input');
		_File_node.type = 'file';
		_File_node.accept = A2($elm$core$String$join, ',', mimes);
		_File_node.addEventListener('change', function(event)
		{
			callback(_Scheduler_succeed(event.target.files[0]));
		});
		_File_click(_File_node);
	});
}

function _File_uploadOneOrMore(mimes)
{
	return _Scheduler_binding(function(callback)
	{
		_File_node = document.createElement('input');
		_File_node.type = 'file';
		_File_node.multiple = true;
		_File_node.accept = A2($elm$core$String$join, ',', mimes);
		_File_node.addEventListener('change', function(event)
		{
			var elmFiles = _List_fromArray(event.target.files);
			callback(_Scheduler_succeed(_Utils_Tuple2(elmFiles.a, elmFiles.b)));
		});
		_File_click(_File_node);
	});
}


// CONTENT

function _File_toString(blob)
{
	return _Scheduler_binding(function(callback)
	{
		var reader = new FileReader();
		reader.addEventListener('loadend', function() {
			callback(_Scheduler_succeed(reader.result));
		});
		reader.readAsText(blob);
		return function() { reader.abort(); };
	});
}

function _File_toBytes(blob)
{
	return _Scheduler_binding(function(callback)
	{
		var reader = new FileReader();
		reader.addEventListener('loadend', function() {
			callback(_Scheduler_succeed(new DataView(reader.result)));
		});
		reader.readAsArrayBuffer(blob);
		return function() { reader.abort(); };
	});
}

function _File_toUrl(blob)
{
	return _Scheduler_binding(function(callback)
	{
		var reader = new FileReader();
		reader.addEventListener('loadend', function() {
			callback(_Scheduler_succeed(reader.result));
		});
		reader.readAsDataURL(blob);
		return function() { reader.abort(); };
	});
}

var $elm$core$List$cons = _List_cons;
var $elm$core$Elm$JsArray$foldr = _JsArray_foldr;
var $elm$core$Array$foldr = F3(
	function (func, baseCase, _v0) {
		var tree = _v0.c;
		var tail = _v0.d;
		var helper = F2(
			function (node, acc) {
				if (!node.$) {
					var subTree = node.a;
					return A3($elm$core$Elm$JsArray$foldr, helper, acc, subTree);
				} else {
					var values = node.a;
					return A3($elm$core$Elm$JsArray$foldr, func, acc, values);
				}
			});
		return A3(
			$elm$core$Elm$JsArray$foldr,
			helper,
			A3($elm$core$Elm$JsArray$foldr, func, baseCase, tail),
			tree);
	});
var $elm$core$Array$toList = function (array) {
	return A3($elm$core$Array$foldr, $elm$core$List$cons, _List_Nil, array);
};
var $elm$core$Dict$foldr = F3(
	function (func, acc, t) {
		foldr:
		while (true) {
			if (t.$ === -2) {
				return acc;
			} else {
				var key = t.b;
				var value = t.c;
				var left = t.d;
				var right = t.e;
				var $temp$func = func,
					$temp$acc = A3(
					func,
					key,
					value,
					A3($elm$core$Dict$foldr, func, acc, right)),
					$temp$t = left;
				func = $temp$func;
				acc = $temp$acc;
				t = $temp$t;
				continue foldr;
			}
		}
	});
var $elm$core$Dict$toList = function (dict) {
	return A3(
		$elm$core$Dict$foldr,
		F3(
			function (key, value, list) {
				return A2(
					$elm$core$List$cons,
					_Utils_Tuple2(key, value),
					list);
			}),
		_List_Nil,
		dict);
};
var $elm$core$Dict$keys = function (dict) {
	return A3(
		$elm$core$Dict$foldr,
		F3(
			function (key, value, keyList) {
				return A2($elm$core$List$cons, key, keyList);
			}),
		_List_Nil,
		dict);
};
var $elm$core$Set$toList = function (_v0) {
	var dict = _v0;
	return $elm$core$Dict$keys(dict);
};
var $elm$core$Basics$EQ = 1;
var $elm$core$Basics$GT = 2;
var $elm$core$Basics$LT = 0;
var $elm$core$Result$Err = function (a) {
	return {$: 1, a: a};
};
var $elm$json$Json$Decode$Failure = F2(
	function (a, b) {
		return {$: 3, a: a, b: b};
	});
var $elm$json$Json$Decode$Field = F2(
	function (a, b) {
		return {$: 0, a: a, b: b};
	});
var $elm$json$Json$Decode$Index = F2(
	function (a, b) {
		return {$: 1, a: a, b: b};
	});
var $elm$core$Result$Ok = function (a) {
	return {$: 0, a: a};
};
var $elm$json$Json$Decode$OneOf = function (a) {
	return {$: 2, a: a};
};
var $elm$core$Basics$False = 1;
var $elm$core$Basics$add = _Basics_add;
var $elm$core$Maybe$Just = function (a) {
	return {$: 0, a: a};
};
var $elm$core$Maybe$Nothing = {$: 1};
var $elm$core$String$all = _String_all;
var $elm$core$Basics$and = _Basics_and;
var $elm$core$Basics$append = _Utils_append;
var $elm$json$Json$Encode$encode = _Json_encode;
var $elm$core$String$fromInt = _String_fromNumber;
var $elm$core$String$join = F2(
	function (sep, chunks) {
		return A2(
			_String_join,
			sep,
			_List_toArray(chunks));
	});
var $elm$core$String$split = F2(
	function (sep, string) {
		return _List_fromArray(
			A2(_String_split, sep, string));
	});
var $elm$json$Json$Decode$indent = function (str) {
	return A2(
		$elm$core$String$join,
		'\n    ',
		A2($elm$core$String$split, '\n', str));
};
var $elm$core$List$foldl = F3(
	function (func, acc, list) {
		foldl:
		while (true) {
			if (!list.b) {
				return acc;
			} else {
				var x = list.a;
				var xs = list.b;
				var $temp$func = func,
					$temp$acc = A2(func, x, acc),
					$temp$list = xs;
				func = $temp$func;
				acc = $temp$acc;
				list = $temp$list;
				continue foldl;
			}
		}
	});
var $elm$core$List$length = function (xs) {
	return A3(
		$elm$core$List$foldl,
		F2(
			function (_v0, i) {
				return i + 1;
			}),
		0,
		xs);
};
var $elm$core$List$map2 = _List_map2;
var $elm$core$Basics$le = _Utils_le;
var $elm$core$Basics$sub = _Basics_sub;
var $elm$core$List$rangeHelp = F3(
	function (lo, hi, list) {
		rangeHelp:
		while (true) {
			if (_Utils_cmp(lo, hi) < 1) {
				var $temp$lo = lo,
					$temp$hi = hi - 1,
					$temp$list = A2($elm$core$List$cons, hi, list);
				lo = $temp$lo;
				hi = $temp$hi;
				list = $temp$list;
				continue rangeHelp;
			} else {
				return list;
			}
		}
	});
var $elm$core$List$range = F2(
	function (lo, hi) {
		return A3($elm$core$List$rangeHelp, lo, hi, _List_Nil);
	});
var $elm$core$List$indexedMap = F2(
	function (f, xs) {
		return A3(
			$elm$core$List$map2,
			f,
			A2(
				$elm$core$List$range,
				0,
				$elm$core$List$length(xs) - 1),
			xs);
	});
var $elm$core$Char$toCode = _Char_toCode;
var $elm$core$Char$isLower = function (_char) {
	var code = $elm$core$Char$toCode(_char);
	return (97 <= code) && (code <= 122);
};
var $elm$core$Char$isUpper = function (_char) {
	var code = $elm$core$Char$toCode(_char);
	return (code <= 90) && (65 <= code);
};
var $elm$core$Basics$or = _Basics_or;
var $elm$core$Char$isAlpha = function (_char) {
	return $elm$core$Char$isLower(_char) || $elm$core$Char$isUpper(_char);
};
var $elm$core$Char$isDigit = function (_char) {
	var code = $elm$core$Char$toCode(_char);
	return (code <= 57) && (48 <= code);
};
var $elm$core$Char$isAlphaNum = function (_char) {
	return $elm$core$Char$isLower(_char) || ($elm$core$Char$isUpper(_char) || $elm$core$Char$isDigit(_char));
};
var $elm$core$List$reverse = function (list) {
	return A3($elm$core$List$foldl, $elm$core$List$cons, _List_Nil, list);
};
var $elm$core$String$uncons = _String_uncons;
var $elm$json$Json$Decode$errorOneOf = F2(
	function (i, error) {
		return '\n\n(' + ($elm$core$String$fromInt(i + 1) + (') ' + $elm$json$Json$Decode$indent(
			$elm$json$Json$Decode$errorToString(error))));
	});
var $elm$json$Json$Decode$errorToString = function (error) {
	return A2($elm$json$Json$Decode$errorToStringHelp, error, _List_Nil);
};
var $elm$json$Json$Decode$errorToStringHelp = F2(
	function (error, context) {
		errorToStringHelp:
		while (true) {
			switch (error.$) {
				case 0:
					var f = error.a;
					var err = error.b;
					var isSimple = function () {
						var _v1 = $elm$core$String$uncons(f);
						if (_v1.$ === 1) {
							return false;
						} else {
							var _v2 = _v1.a;
							var _char = _v2.a;
							var rest = _v2.b;
							return $elm$core$Char$isAlpha(_char) && A2($elm$core$String$all, $elm$core$Char$isAlphaNum, rest);
						}
					}();
					var fieldName = isSimple ? ('.' + f) : ('[\'' + (f + '\']'));
					var $temp$error = err,
						$temp$context = A2($elm$core$List$cons, fieldName, context);
					error = $temp$error;
					context = $temp$context;
					continue errorToStringHelp;
				case 1:
					var i = error.a;
					var err = error.b;
					var indexName = '[' + ($elm$core$String$fromInt(i) + ']');
					var $temp$error = err,
						$temp$context = A2($elm$core$List$cons, indexName, context);
					error = $temp$error;
					context = $temp$context;
					continue errorToStringHelp;
				case 2:
					var errors = error.a;
					if (!errors.b) {
						return 'Ran into a Json.Decode.oneOf with no possibilities' + function () {
							if (!context.b) {
								return '!';
							} else {
								return ' at json' + A2(
									$elm$core$String$join,
									'',
									$elm$core$List$reverse(context));
							}
						}();
					} else {
						if (!errors.b.b) {
							var err = errors.a;
							var $temp$error = err,
								$temp$context = context;
							error = $temp$error;
							context = $temp$context;
							continue errorToStringHelp;
						} else {
							var starter = function () {
								if (!context.b) {
									return 'Json.Decode.oneOf';
								} else {
									return 'The Json.Decode.oneOf at json' + A2(
										$elm$core$String$join,
										'',
										$elm$core$List$reverse(context));
								}
							}();
							var introduction = starter + (' failed in the following ' + ($elm$core$String$fromInt(
								$elm$core$List$length(errors)) + ' ways:'));
							return A2(
								$elm$core$String$join,
								'\n\n',
								A2(
									$elm$core$List$cons,
									introduction,
									A2($elm$core$List$indexedMap, $elm$json$Json$Decode$errorOneOf, errors)));
						}
					}
				default:
					var msg = error.a;
					var json = error.b;
					var introduction = function () {
						if (!context.b) {
							return 'Problem with the given value:\n\n';
						} else {
							return 'Problem with the value at json' + (A2(
								$elm$core$String$join,
								'',
								$elm$core$List$reverse(context)) + ':\n\n    ');
						}
					}();
					return introduction + ($elm$json$Json$Decode$indent(
						A2($elm$json$Json$Encode$encode, 4, json)) + ('\n\n' + msg));
			}
		}
	});
var $elm$core$Array$branchFactor = 32;
var $elm$core$Array$Array_elm_builtin = F4(
	function (a, b, c, d) {
		return {$: 0, a: a, b: b, c: c, d: d};
	});
var $elm$core$Elm$JsArray$empty = _JsArray_empty;
var $elm$core$Basics$ceiling = _Basics_ceiling;
var $elm$core$Basics$fdiv = _Basics_fdiv;
var $elm$core$Basics$logBase = F2(
	function (base, number) {
		return _Basics_log(number) / _Basics_log(base);
	});
var $elm$core$Basics$toFloat = _Basics_toFloat;
var $elm$core$Array$shiftStep = $elm$core$Basics$ceiling(
	A2($elm$core$Basics$logBase, 2, $elm$core$Array$branchFactor));
var $elm$core$Array$empty = A4($elm$core$Array$Array_elm_builtin, 0, $elm$core$Array$shiftStep, $elm$core$Elm$JsArray$empty, $elm$core$Elm$JsArray$empty);
var $elm$core$Elm$JsArray$initialize = _JsArray_initialize;
var $elm$core$Array$Leaf = function (a) {
	return {$: 1, a: a};
};
var $elm$core$Basics$apL = F2(
	function (f, x) {
		return f(x);
	});
var $elm$core$Basics$apR = F2(
	function (x, f) {
		return f(x);
	});
var $elm$core$Basics$eq = _Utils_equal;
var $elm$core$Basics$floor = _Basics_floor;
var $elm$core$Elm$JsArray$length = _JsArray_length;
var $elm$core$Basics$gt = _Utils_gt;
var $elm$core$Basics$max = F2(
	function (x, y) {
		return (_Utils_cmp(x, y) > 0) ? x : y;
	});
var $elm$core$Basics$mul = _Basics_mul;
var $elm$core$Array$SubTree = function (a) {
	return {$: 0, a: a};
};
var $elm$core$Elm$JsArray$initializeFromList = _JsArray_initializeFromList;
var $elm$core$Array$compressNodes = F2(
	function (nodes, acc) {
		compressNodes:
		while (true) {
			var _v0 = A2($elm$core$Elm$JsArray$initializeFromList, $elm$core$Array$branchFactor, nodes);
			var node = _v0.a;
			var remainingNodes = _v0.b;
			var newAcc = A2(
				$elm$core$List$cons,
				$elm$core$Array$SubTree(node),
				acc);
			if (!remainingNodes.b) {
				return $elm$core$List$reverse(newAcc);
			} else {
				var $temp$nodes = remainingNodes,
					$temp$acc = newAcc;
				nodes = $temp$nodes;
				acc = $temp$acc;
				continue compressNodes;
			}
		}
	});
var $elm$core$Tuple$first = function (_v0) {
	var x = _v0.a;
	return x;
};
var $elm$core$Array$treeFromBuilder = F2(
	function (nodeList, nodeListSize) {
		treeFromBuilder:
		while (true) {
			var newNodeSize = $elm$core$Basics$ceiling(nodeListSize / $elm$core$Array$branchFactor);
			if (newNodeSize === 1) {
				return A2($elm$core$Elm$JsArray$initializeFromList, $elm$core$Array$branchFactor, nodeList).a;
			} else {
				var $temp$nodeList = A2($elm$core$Array$compressNodes, nodeList, _List_Nil),
					$temp$nodeListSize = newNodeSize;
				nodeList = $temp$nodeList;
				nodeListSize = $temp$nodeListSize;
				continue treeFromBuilder;
			}
		}
	});
var $elm$core$Array$builderToArray = F2(
	function (reverseNodeList, builder) {
		if (!builder.p) {
			return A4(
				$elm$core$Array$Array_elm_builtin,
				$elm$core$Elm$JsArray$length(builder.t),
				$elm$core$Array$shiftStep,
				$elm$core$Elm$JsArray$empty,
				builder.t);
		} else {
			var treeLen = builder.p * $elm$core$Array$branchFactor;
			var depth = $elm$core$Basics$floor(
				A2($elm$core$Basics$logBase, $elm$core$Array$branchFactor, treeLen - 1));
			var correctNodeList = reverseNodeList ? $elm$core$List$reverse(builder.w) : builder.w;
			var tree = A2($elm$core$Array$treeFromBuilder, correctNodeList, builder.p);
			return A4(
				$elm$core$Array$Array_elm_builtin,
				$elm$core$Elm$JsArray$length(builder.t) + treeLen,
				A2($elm$core$Basics$max, 5, depth * $elm$core$Array$shiftStep),
				tree,
				builder.t);
		}
	});
var $elm$core$Basics$idiv = _Basics_idiv;
var $elm$core$Basics$lt = _Utils_lt;
var $elm$core$Array$initializeHelp = F5(
	function (fn, fromIndex, len, nodeList, tail) {
		initializeHelp:
		while (true) {
			if (fromIndex < 0) {
				return A2(
					$elm$core$Array$builderToArray,
					false,
					{w: nodeList, p: (len / $elm$core$Array$branchFactor) | 0, t: tail});
			} else {
				var leaf = $elm$core$Array$Leaf(
					A3($elm$core$Elm$JsArray$initialize, $elm$core$Array$branchFactor, fromIndex, fn));
				var $temp$fn = fn,
					$temp$fromIndex = fromIndex - $elm$core$Array$branchFactor,
					$temp$len = len,
					$temp$nodeList = A2($elm$core$List$cons, leaf, nodeList),
					$temp$tail = tail;
				fn = $temp$fn;
				fromIndex = $temp$fromIndex;
				len = $temp$len;
				nodeList = $temp$nodeList;
				tail = $temp$tail;
				continue initializeHelp;
			}
		}
	});
var $elm$core$Basics$remainderBy = _Basics_remainderBy;
var $elm$core$Array$initialize = F2(
	function (len, fn) {
		if (len <= 0) {
			return $elm$core$Array$empty;
		} else {
			var tailLen = len % $elm$core$Array$branchFactor;
			var tail = A3($elm$core$Elm$JsArray$initialize, tailLen, len - tailLen, fn);
			var initialFromIndex = (len - tailLen) - $elm$core$Array$branchFactor;
			return A5($elm$core$Array$initializeHelp, fn, initialFromIndex, len, _List_Nil, tail);
		}
	});
var $elm$core$Basics$True = 0;
var $elm$core$Result$isOk = function (result) {
	if (!result.$) {
		return true;
	} else {
		return false;
	}
};
var $elm$json$Json$Decode$andThen = _Json_andThen;
var $elm$json$Json$Decode$map = _Json_map1;
var $elm$json$Json$Decode$map2 = _Json_map2;
var $elm$json$Json$Decode$succeed = _Json_succeed;
var $elm$virtual_dom$VirtualDom$toHandlerInt = function (handler) {
	switch (handler.$) {
		case 0:
			return 0;
		case 1:
			return 1;
		case 2:
			return 2;
		default:
			return 3;
	}
};
var $elm$browser$Browser$External = function (a) {
	return {$: 1, a: a};
};
var $elm$browser$Browser$Internal = function (a) {
	return {$: 0, a: a};
};
var $elm$core$Basics$identity = function (x) {
	return x;
};
var $elm$browser$Browser$Dom$NotFound = $elm$core$Basics$identity;
var $elm$url$Url$Http = 0;
var $elm$url$Url$Https = 1;
var $elm$url$Url$Url = F6(
	function (protocol, host, port_, path, query, fragment) {
		return {bh: fragment, bj: host, bp: path, br: port_, bu: protocol, bv: query};
	});
var $elm$core$String$contains = _String_contains;
var $elm$core$String$length = _String_length;
var $elm$core$String$slice = _String_slice;
var $elm$core$String$dropLeft = F2(
	function (n, string) {
		return (n < 1) ? string : A3(
			$elm$core$String$slice,
			n,
			$elm$core$String$length(string),
			string);
	});
var $elm$core$String$indexes = _String_indexes;
var $elm$core$String$isEmpty = function (string) {
	return string === '';
};
var $elm$core$String$left = F2(
	function (n, string) {
		return (n < 1) ? '' : A3($elm$core$String$slice, 0, n, string);
	});
var $elm$core$String$toInt = _String_toInt;
var $elm$url$Url$chompBeforePath = F5(
	function (protocol, path, params, frag, str) {
		if ($elm$core$String$isEmpty(str) || A2($elm$core$String$contains, '@', str)) {
			return $elm$core$Maybe$Nothing;
		} else {
			var _v0 = A2($elm$core$String$indexes, ':', str);
			if (!_v0.b) {
				return $elm$core$Maybe$Just(
					A6($elm$url$Url$Url, protocol, str, $elm$core$Maybe$Nothing, path, params, frag));
			} else {
				if (!_v0.b.b) {
					var i = _v0.a;
					var _v1 = $elm$core$String$toInt(
						A2($elm$core$String$dropLeft, i + 1, str));
					if (_v1.$ === 1) {
						return $elm$core$Maybe$Nothing;
					} else {
						var port_ = _v1;
						return $elm$core$Maybe$Just(
							A6(
								$elm$url$Url$Url,
								protocol,
								A2($elm$core$String$left, i, str),
								port_,
								path,
								params,
								frag));
					}
				} else {
					return $elm$core$Maybe$Nothing;
				}
			}
		}
	});
var $elm$url$Url$chompBeforeQuery = F4(
	function (protocol, params, frag, str) {
		if ($elm$core$String$isEmpty(str)) {
			return $elm$core$Maybe$Nothing;
		} else {
			var _v0 = A2($elm$core$String$indexes, '/', str);
			if (!_v0.b) {
				return A5($elm$url$Url$chompBeforePath, protocol, '/', params, frag, str);
			} else {
				var i = _v0.a;
				return A5(
					$elm$url$Url$chompBeforePath,
					protocol,
					A2($elm$core$String$dropLeft, i, str),
					params,
					frag,
					A2($elm$core$String$left, i, str));
			}
		}
	});
var $elm$url$Url$chompBeforeFragment = F3(
	function (protocol, frag, str) {
		if ($elm$core$String$isEmpty(str)) {
			return $elm$core$Maybe$Nothing;
		} else {
			var _v0 = A2($elm$core$String$indexes, '?', str);
			if (!_v0.b) {
				return A4($elm$url$Url$chompBeforeQuery, protocol, $elm$core$Maybe$Nothing, frag, str);
			} else {
				var i = _v0.a;
				return A4(
					$elm$url$Url$chompBeforeQuery,
					protocol,
					$elm$core$Maybe$Just(
						A2($elm$core$String$dropLeft, i + 1, str)),
					frag,
					A2($elm$core$String$left, i, str));
			}
		}
	});
var $elm$url$Url$chompAfterProtocol = F2(
	function (protocol, str) {
		if ($elm$core$String$isEmpty(str)) {
			return $elm$core$Maybe$Nothing;
		} else {
			var _v0 = A2($elm$core$String$indexes, '#', str);
			if (!_v0.b) {
				return A3($elm$url$Url$chompBeforeFragment, protocol, $elm$core$Maybe$Nothing, str);
			} else {
				var i = _v0.a;
				return A3(
					$elm$url$Url$chompBeforeFragment,
					protocol,
					$elm$core$Maybe$Just(
						A2($elm$core$String$dropLeft, i + 1, str)),
					A2($elm$core$String$left, i, str));
			}
		}
	});
var $elm$core$String$startsWith = _String_startsWith;
var $elm$url$Url$fromString = function (str) {
	return A2($elm$core$String$startsWith, 'http://', str) ? A2(
		$elm$url$Url$chompAfterProtocol,
		0,
		A2($elm$core$String$dropLeft, 7, str)) : (A2($elm$core$String$startsWith, 'https://', str) ? A2(
		$elm$url$Url$chompAfterProtocol,
		1,
		A2($elm$core$String$dropLeft, 8, str)) : $elm$core$Maybe$Nothing);
};
var $elm$core$Basics$never = function (_v0) {
	never:
	while (true) {
		var nvr = _v0;
		var $temp$_v0 = nvr;
		_v0 = $temp$_v0;
		continue never;
	}
};
var $elm$core$Task$Perform = $elm$core$Basics$identity;
var $elm$core$Task$succeed = _Scheduler_succeed;
var $elm$core$Task$init = $elm$core$Task$succeed(0);
var $elm$core$List$foldrHelper = F4(
	function (fn, acc, ctr, ls) {
		if (!ls.b) {
			return acc;
		} else {
			var a = ls.a;
			var r1 = ls.b;
			if (!r1.b) {
				return A2(fn, a, acc);
			} else {
				var b = r1.a;
				var r2 = r1.b;
				if (!r2.b) {
					return A2(
						fn,
						a,
						A2(fn, b, acc));
				} else {
					var c = r2.a;
					var r3 = r2.b;
					if (!r3.b) {
						return A2(
							fn,
							a,
							A2(
								fn,
								b,
								A2(fn, c, acc)));
					} else {
						var d = r3.a;
						var r4 = r3.b;
						var res = (ctr > 500) ? A3(
							$elm$core$List$foldl,
							fn,
							acc,
							$elm$core$List$reverse(r4)) : A4($elm$core$List$foldrHelper, fn, acc, ctr + 1, r4);
						return A2(
							fn,
							a,
							A2(
								fn,
								b,
								A2(
									fn,
									c,
									A2(fn, d, res))));
					}
				}
			}
		}
	});
var $elm$core$List$foldr = F3(
	function (fn, acc, ls) {
		return A4($elm$core$List$foldrHelper, fn, acc, 0, ls);
	});
var $elm$core$List$map = F2(
	function (f, xs) {
		return A3(
			$elm$core$List$foldr,
			F2(
				function (x, acc) {
					return A2(
						$elm$core$List$cons,
						f(x),
						acc);
				}),
			_List_Nil,
			xs);
	});
var $elm$core$Task$andThen = _Scheduler_andThen;
var $elm$core$Task$map = F2(
	function (func, taskA) {
		return A2(
			$elm$core$Task$andThen,
			function (a) {
				return $elm$core$Task$succeed(
					func(a));
			},
			taskA);
	});
var $elm$core$Task$map2 = F3(
	function (func, taskA, taskB) {
		return A2(
			$elm$core$Task$andThen,
			function (a) {
				return A2(
					$elm$core$Task$andThen,
					function (b) {
						return $elm$core$Task$succeed(
							A2(func, a, b));
					},
					taskB);
			},
			taskA);
	});
var $elm$core$Task$sequence = function (tasks) {
	return A3(
		$elm$core$List$foldr,
		$elm$core$Task$map2($elm$core$List$cons),
		$elm$core$Task$succeed(_List_Nil),
		tasks);
};
var $elm$core$Platform$sendToApp = _Platform_sendToApp;
var $elm$core$Task$spawnCmd = F2(
	function (router, _v0) {
		var task = _v0;
		return _Scheduler_spawn(
			A2(
				$elm$core$Task$andThen,
				$elm$core$Platform$sendToApp(router),
				task));
	});
var $elm$core$Task$onEffects = F3(
	function (router, commands, state) {
		return A2(
			$elm$core$Task$map,
			function (_v0) {
				return 0;
			},
			$elm$core$Task$sequence(
				A2(
					$elm$core$List$map,
					$elm$core$Task$spawnCmd(router),
					commands)));
	});
var $elm$core$Task$onSelfMsg = F3(
	function (_v0, _v1, _v2) {
		return $elm$core$Task$succeed(0);
	});
var $elm$core$Task$cmdMap = F2(
	function (tagger, _v0) {
		var task = _v0;
		return A2($elm$core$Task$map, tagger, task);
	});
_Platform_effectManagers['Task'] = _Platform_createManager($elm$core$Task$init, $elm$core$Task$onEffects, $elm$core$Task$onSelfMsg, $elm$core$Task$cmdMap);
var $elm$core$Task$command = _Platform_leaf('Task');
var $elm$core$Task$perform = F2(
	function (toMessage, task) {
		return $elm$core$Task$command(
			A2($elm$core$Task$map, toMessage, task));
	});
var $elm$browser$Browser$element = _Browser_element;
var $elm$json$Json$Decode$field = _Json_decodeField;
var $author$project$Main$GotViewport = function (a) {
	return {$: 12, a: a};
};
var $author$project$Main$Idle = {$: 0};
var $author$project$Main$ModeInit = 0;
var $author$project$Main$NotGenerated = 0;
var $elm$core$Platform$Cmd$batch = _Platform_batch;
var $elm$core$Dict$RBEmpty_elm_builtin = {$: -2};
var $elm$core$Dict$empty = $elm$core$Dict$RBEmpty_elm_builtin;
var $author$project$Main$GotFileList = function (a) {
	return {$: 0, a: a};
};
var $elm$json$Json$Decode$string = _Json_decodeString;
var $author$project$Main$decodePdfFile = A3(
	$elm$json$Json$Decode$map2,
	F2(
		function (n, p) {
			return {N: n, bp: p};
		}),
	A2($elm$json$Json$Decode$field, 'name', $elm$json$Json$Decode$string),
	A2($elm$json$Json$Decode$field, 'path', $elm$json$Json$Decode$string));
var $elm$json$Json$Decode$decodeString = _Json_runOnString;
var $elm$http$Http$BadStatus_ = F2(
	function (a, b) {
		return {$: 3, a: a, b: b};
	});
var $elm$http$Http$BadUrl_ = function (a) {
	return {$: 0, a: a};
};
var $elm$http$Http$GoodStatus_ = F2(
	function (a, b) {
		return {$: 4, a: a, b: b};
	});
var $elm$http$Http$NetworkError_ = {$: 2};
var $elm$http$Http$Receiving = function (a) {
	return {$: 1, a: a};
};
var $elm$http$Http$Sending = function (a) {
	return {$: 0, a: a};
};
var $elm$http$Http$Timeout_ = {$: 1};
var $elm$core$Maybe$isJust = function (maybe) {
	if (!maybe.$) {
		return true;
	} else {
		return false;
	}
};
var $elm$core$Platform$sendToSelf = _Platform_sendToSelf;
var $elm$core$Basics$compare = _Utils_compare;
var $elm$core$Dict$get = F2(
	function (targetKey, dict) {
		get:
		while (true) {
			if (dict.$ === -2) {
				return $elm$core$Maybe$Nothing;
			} else {
				var key = dict.b;
				var value = dict.c;
				var left = dict.d;
				var right = dict.e;
				var _v1 = A2($elm$core$Basics$compare, targetKey, key);
				switch (_v1) {
					case 0:
						var $temp$targetKey = targetKey,
							$temp$dict = left;
						targetKey = $temp$targetKey;
						dict = $temp$dict;
						continue get;
					case 1:
						return $elm$core$Maybe$Just(value);
					default:
						var $temp$targetKey = targetKey,
							$temp$dict = right;
						targetKey = $temp$targetKey;
						dict = $temp$dict;
						continue get;
				}
			}
		}
	});
var $elm$core$Dict$Black = 1;
var $elm$core$Dict$RBNode_elm_builtin = F5(
	function (a, b, c, d, e) {
		return {$: -1, a: a, b: b, c: c, d: d, e: e};
	});
var $elm$core$Dict$Red = 0;
var $elm$core$Dict$balance = F5(
	function (color, key, value, left, right) {
		if ((right.$ === -1) && (!right.a)) {
			var _v1 = right.a;
			var rK = right.b;
			var rV = right.c;
			var rLeft = right.d;
			var rRight = right.e;
			if ((left.$ === -1) && (!left.a)) {
				var _v3 = left.a;
				var lK = left.b;
				var lV = left.c;
				var lLeft = left.d;
				var lRight = left.e;
				return A5(
					$elm$core$Dict$RBNode_elm_builtin,
					0,
					key,
					value,
					A5($elm$core$Dict$RBNode_elm_builtin, 1, lK, lV, lLeft, lRight),
					A5($elm$core$Dict$RBNode_elm_builtin, 1, rK, rV, rLeft, rRight));
			} else {
				return A5(
					$elm$core$Dict$RBNode_elm_builtin,
					color,
					rK,
					rV,
					A5($elm$core$Dict$RBNode_elm_builtin, 0, key, value, left, rLeft),
					rRight);
			}
		} else {
			if ((((left.$ === -1) && (!left.a)) && (left.d.$ === -1)) && (!left.d.a)) {
				var _v5 = left.a;
				var lK = left.b;
				var lV = left.c;
				var _v6 = left.d;
				var _v7 = _v6.a;
				var llK = _v6.b;
				var llV = _v6.c;
				var llLeft = _v6.d;
				var llRight = _v6.e;
				var lRight = left.e;
				return A5(
					$elm$core$Dict$RBNode_elm_builtin,
					0,
					lK,
					lV,
					A5($elm$core$Dict$RBNode_elm_builtin, 1, llK, llV, llLeft, llRight),
					A5($elm$core$Dict$RBNode_elm_builtin, 1, key, value, lRight, right));
			} else {
				return A5($elm$core$Dict$RBNode_elm_builtin, color, key, value, left, right);
			}
		}
	});
var $elm$core$Dict$insertHelp = F3(
	function (key, value, dict) {
		if (dict.$ === -2) {
			return A5($elm$core$Dict$RBNode_elm_builtin, 0, key, value, $elm$core$Dict$RBEmpty_elm_builtin, $elm$core$Dict$RBEmpty_elm_builtin);
		} else {
			var nColor = dict.a;
			var nKey = dict.b;
			var nValue = dict.c;
			var nLeft = dict.d;
			var nRight = dict.e;
			var _v1 = A2($elm$core$Basics$compare, key, nKey);
			switch (_v1) {
				case 0:
					return A5(
						$elm$core$Dict$balance,
						nColor,
						nKey,
						nValue,
						A3($elm$core$Dict$insertHelp, key, value, nLeft),
						nRight);
				case 1:
					return A5($elm$core$Dict$RBNode_elm_builtin, nColor, nKey, value, nLeft, nRight);
				default:
					return A5(
						$elm$core$Dict$balance,
						nColor,
						nKey,
						nValue,
						nLeft,
						A3($elm$core$Dict$insertHelp, key, value, nRight));
			}
		}
	});
var $elm$core$Dict$insert = F3(
	function (key, value, dict) {
		var _v0 = A3($elm$core$Dict$insertHelp, key, value, dict);
		if ((_v0.$ === -1) && (!_v0.a)) {
			var _v1 = _v0.a;
			var k = _v0.b;
			var v = _v0.c;
			var l = _v0.d;
			var r = _v0.e;
			return A5($elm$core$Dict$RBNode_elm_builtin, 1, k, v, l, r);
		} else {
			var x = _v0;
			return x;
		}
	});
var $elm$core$Dict$getMin = function (dict) {
	getMin:
	while (true) {
		if ((dict.$ === -1) && (dict.d.$ === -1)) {
			var left = dict.d;
			var $temp$dict = left;
			dict = $temp$dict;
			continue getMin;
		} else {
			return dict;
		}
	}
};
var $elm$core$Dict$moveRedLeft = function (dict) {
	if (((dict.$ === -1) && (dict.d.$ === -1)) && (dict.e.$ === -1)) {
		if ((dict.e.d.$ === -1) && (!dict.e.d.a)) {
			var clr = dict.a;
			var k = dict.b;
			var v = dict.c;
			var _v1 = dict.d;
			var lClr = _v1.a;
			var lK = _v1.b;
			var lV = _v1.c;
			var lLeft = _v1.d;
			var lRight = _v1.e;
			var _v2 = dict.e;
			var rClr = _v2.a;
			var rK = _v2.b;
			var rV = _v2.c;
			var rLeft = _v2.d;
			var _v3 = rLeft.a;
			var rlK = rLeft.b;
			var rlV = rLeft.c;
			var rlL = rLeft.d;
			var rlR = rLeft.e;
			var rRight = _v2.e;
			return A5(
				$elm$core$Dict$RBNode_elm_builtin,
				0,
				rlK,
				rlV,
				A5(
					$elm$core$Dict$RBNode_elm_builtin,
					1,
					k,
					v,
					A5($elm$core$Dict$RBNode_elm_builtin, 0, lK, lV, lLeft, lRight),
					rlL),
				A5($elm$core$Dict$RBNode_elm_builtin, 1, rK, rV, rlR, rRight));
		} else {
			var clr = dict.a;
			var k = dict.b;
			var v = dict.c;
			var _v4 = dict.d;
			var lClr = _v4.a;
			var lK = _v4.b;
			var lV = _v4.c;
			var lLeft = _v4.d;
			var lRight = _v4.e;
			var _v5 = dict.e;
			var rClr = _v5.a;
			var rK = _v5.b;
			var rV = _v5.c;
			var rLeft = _v5.d;
			var rRight = _v5.e;
			if (clr === 1) {
				return A5(
					$elm$core$Dict$RBNode_elm_builtin,
					1,
					k,
					v,
					A5($elm$core$Dict$RBNode_elm_builtin, 0, lK, lV, lLeft, lRight),
					A5($elm$core$Dict$RBNode_elm_builtin, 0, rK, rV, rLeft, rRight));
			} else {
				return A5(
					$elm$core$Dict$RBNode_elm_builtin,
					1,
					k,
					v,
					A5($elm$core$Dict$RBNode_elm_builtin, 0, lK, lV, lLeft, lRight),
					A5($elm$core$Dict$RBNode_elm_builtin, 0, rK, rV, rLeft, rRight));
			}
		}
	} else {
		return dict;
	}
};
var $elm$core$Dict$moveRedRight = function (dict) {
	if (((dict.$ === -1) && (dict.d.$ === -1)) && (dict.e.$ === -1)) {
		if ((dict.d.d.$ === -1) && (!dict.d.d.a)) {
			var clr = dict.a;
			var k = dict.b;
			var v = dict.c;
			var _v1 = dict.d;
			var lClr = _v1.a;
			var lK = _v1.b;
			var lV = _v1.c;
			var _v2 = _v1.d;
			var _v3 = _v2.a;
			var llK = _v2.b;
			var llV = _v2.c;
			var llLeft = _v2.d;
			var llRight = _v2.e;
			var lRight = _v1.e;
			var _v4 = dict.e;
			var rClr = _v4.a;
			var rK = _v4.b;
			var rV = _v4.c;
			var rLeft = _v4.d;
			var rRight = _v4.e;
			return A5(
				$elm$core$Dict$RBNode_elm_builtin,
				0,
				lK,
				lV,
				A5($elm$core$Dict$RBNode_elm_builtin, 1, llK, llV, llLeft, llRight),
				A5(
					$elm$core$Dict$RBNode_elm_builtin,
					1,
					k,
					v,
					lRight,
					A5($elm$core$Dict$RBNode_elm_builtin, 0, rK, rV, rLeft, rRight)));
		} else {
			var clr = dict.a;
			var k = dict.b;
			var v = dict.c;
			var _v5 = dict.d;
			var lClr = _v5.a;
			var lK = _v5.b;
			var lV = _v5.c;
			var lLeft = _v5.d;
			var lRight = _v5.e;
			var _v6 = dict.e;
			var rClr = _v6.a;
			var rK = _v6.b;
			var rV = _v6.c;
			var rLeft = _v6.d;
			var rRight = _v6.e;
			if (clr === 1) {
				return A5(
					$elm$core$Dict$RBNode_elm_builtin,
					1,
					k,
					v,
					A5($elm$core$Dict$RBNode_elm_builtin, 0, lK, lV, lLeft, lRight),
					A5($elm$core$Dict$RBNode_elm_builtin, 0, rK, rV, rLeft, rRight));
			} else {
				return A5(
					$elm$core$Dict$RBNode_elm_builtin,
					1,
					k,
					v,
					A5($elm$core$Dict$RBNode_elm_builtin, 0, lK, lV, lLeft, lRight),
					A5($elm$core$Dict$RBNode_elm_builtin, 0, rK, rV, rLeft, rRight));
			}
		}
	} else {
		return dict;
	}
};
var $elm$core$Dict$removeHelpPrepEQGT = F7(
	function (targetKey, dict, color, key, value, left, right) {
		if ((left.$ === -1) && (!left.a)) {
			var _v1 = left.a;
			var lK = left.b;
			var lV = left.c;
			var lLeft = left.d;
			var lRight = left.e;
			return A5(
				$elm$core$Dict$RBNode_elm_builtin,
				color,
				lK,
				lV,
				lLeft,
				A5($elm$core$Dict$RBNode_elm_builtin, 0, key, value, lRight, right));
		} else {
			_v2$2:
			while (true) {
				if ((right.$ === -1) && (right.a === 1)) {
					if (right.d.$ === -1) {
						if (right.d.a === 1) {
							var _v3 = right.a;
							var _v4 = right.d;
							var _v5 = _v4.a;
							return $elm$core$Dict$moveRedRight(dict);
						} else {
							break _v2$2;
						}
					} else {
						var _v6 = right.a;
						var _v7 = right.d;
						return $elm$core$Dict$moveRedRight(dict);
					}
				} else {
					break _v2$2;
				}
			}
			return dict;
		}
	});
var $elm$core$Dict$removeMin = function (dict) {
	if ((dict.$ === -1) && (dict.d.$ === -1)) {
		var color = dict.a;
		var key = dict.b;
		var value = dict.c;
		var left = dict.d;
		var lColor = left.a;
		var lLeft = left.d;
		var right = dict.e;
		if (lColor === 1) {
			if ((lLeft.$ === -1) && (!lLeft.a)) {
				var _v3 = lLeft.a;
				return A5(
					$elm$core$Dict$RBNode_elm_builtin,
					color,
					key,
					value,
					$elm$core$Dict$removeMin(left),
					right);
			} else {
				var _v4 = $elm$core$Dict$moveRedLeft(dict);
				if (_v4.$ === -1) {
					var nColor = _v4.a;
					var nKey = _v4.b;
					var nValue = _v4.c;
					var nLeft = _v4.d;
					var nRight = _v4.e;
					return A5(
						$elm$core$Dict$balance,
						nColor,
						nKey,
						nValue,
						$elm$core$Dict$removeMin(nLeft),
						nRight);
				} else {
					return $elm$core$Dict$RBEmpty_elm_builtin;
				}
			}
		} else {
			return A5(
				$elm$core$Dict$RBNode_elm_builtin,
				color,
				key,
				value,
				$elm$core$Dict$removeMin(left),
				right);
		}
	} else {
		return $elm$core$Dict$RBEmpty_elm_builtin;
	}
};
var $elm$core$Dict$removeHelp = F2(
	function (targetKey, dict) {
		if (dict.$ === -2) {
			return $elm$core$Dict$RBEmpty_elm_builtin;
		} else {
			var color = dict.a;
			var key = dict.b;
			var value = dict.c;
			var left = dict.d;
			var right = dict.e;
			if (_Utils_cmp(targetKey, key) < 0) {
				if ((left.$ === -1) && (left.a === 1)) {
					var _v4 = left.a;
					var lLeft = left.d;
					if ((lLeft.$ === -1) && (!lLeft.a)) {
						var _v6 = lLeft.a;
						return A5(
							$elm$core$Dict$RBNode_elm_builtin,
							color,
							key,
							value,
							A2($elm$core$Dict$removeHelp, targetKey, left),
							right);
					} else {
						var _v7 = $elm$core$Dict$moveRedLeft(dict);
						if (_v7.$ === -1) {
							var nColor = _v7.a;
							var nKey = _v7.b;
							var nValue = _v7.c;
							var nLeft = _v7.d;
							var nRight = _v7.e;
							return A5(
								$elm$core$Dict$balance,
								nColor,
								nKey,
								nValue,
								A2($elm$core$Dict$removeHelp, targetKey, nLeft),
								nRight);
						} else {
							return $elm$core$Dict$RBEmpty_elm_builtin;
						}
					}
				} else {
					return A5(
						$elm$core$Dict$RBNode_elm_builtin,
						color,
						key,
						value,
						A2($elm$core$Dict$removeHelp, targetKey, left),
						right);
				}
			} else {
				return A2(
					$elm$core$Dict$removeHelpEQGT,
					targetKey,
					A7($elm$core$Dict$removeHelpPrepEQGT, targetKey, dict, color, key, value, left, right));
			}
		}
	});
var $elm$core$Dict$removeHelpEQGT = F2(
	function (targetKey, dict) {
		if (dict.$ === -1) {
			var color = dict.a;
			var key = dict.b;
			var value = dict.c;
			var left = dict.d;
			var right = dict.e;
			if (_Utils_eq(targetKey, key)) {
				var _v1 = $elm$core$Dict$getMin(right);
				if (_v1.$ === -1) {
					var minKey = _v1.b;
					var minValue = _v1.c;
					return A5(
						$elm$core$Dict$balance,
						color,
						minKey,
						minValue,
						left,
						$elm$core$Dict$removeMin(right));
				} else {
					return $elm$core$Dict$RBEmpty_elm_builtin;
				}
			} else {
				return A5(
					$elm$core$Dict$balance,
					color,
					key,
					value,
					left,
					A2($elm$core$Dict$removeHelp, targetKey, right));
			}
		} else {
			return $elm$core$Dict$RBEmpty_elm_builtin;
		}
	});
var $elm$core$Dict$remove = F2(
	function (key, dict) {
		var _v0 = A2($elm$core$Dict$removeHelp, key, dict);
		if ((_v0.$ === -1) && (!_v0.a)) {
			var _v1 = _v0.a;
			var k = _v0.b;
			var v = _v0.c;
			var l = _v0.d;
			var r = _v0.e;
			return A5($elm$core$Dict$RBNode_elm_builtin, 1, k, v, l, r);
		} else {
			var x = _v0;
			return x;
		}
	});
var $elm$core$Dict$update = F3(
	function (targetKey, alter, dictionary) {
		var _v0 = alter(
			A2($elm$core$Dict$get, targetKey, dictionary));
		if (!_v0.$) {
			var value = _v0.a;
			return A3($elm$core$Dict$insert, targetKey, value, dictionary);
		} else {
			return A2($elm$core$Dict$remove, targetKey, dictionary);
		}
	});
var $elm$core$Basics$composeR = F3(
	function (f, g, x) {
		return g(
			f(x));
	});
var $elm$http$Http$expectStringResponse = F2(
	function (toMsg, toResult) {
		return A3(
			_Http_expect,
			'',
			$elm$core$Basics$identity,
			A2($elm$core$Basics$composeR, toResult, toMsg));
	});
var $elm$core$Result$mapError = F2(
	function (f, result) {
		if (!result.$) {
			var v = result.a;
			return $elm$core$Result$Ok(v);
		} else {
			var e = result.a;
			return $elm$core$Result$Err(
				f(e));
		}
	});
var $elm$http$Http$BadBody = function (a) {
	return {$: 4, a: a};
};
var $elm$http$Http$BadStatus = function (a) {
	return {$: 3, a: a};
};
var $elm$http$Http$BadUrl = function (a) {
	return {$: 0, a: a};
};
var $elm$http$Http$NetworkError = {$: 2};
var $elm$http$Http$Timeout = {$: 1};
var $elm$http$Http$resolve = F2(
	function (toResult, response) {
		switch (response.$) {
			case 0:
				var url = response.a;
				return $elm$core$Result$Err(
					$elm$http$Http$BadUrl(url));
			case 1:
				return $elm$core$Result$Err($elm$http$Http$Timeout);
			case 2:
				return $elm$core$Result$Err($elm$http$Http$NetworkError);
			case 3:
				var metadata = response.a;
				return $elm$core$Result$Err(
					$elm$http$Http$BadStatus(metadata.b1));
			default:
				var body = response.b;
				return A2(
					$elm$core$Result$mapError,
					$elm$http$Http$BadBody,
					toResult(body));
		}
	});
var $elm$http$Http$expectJson = F2(
	function (toMsg, decoder) {
		return A2(
			$elm$http$Http$expectStringResponse,
			toMsg,
			$elm$http$Http$resolve(
				function (string) {
					return A2(
						$elm$core$Result$mapError,
						$elm$json$Json$Decode$errorToString,
						A2($elm$json$Json$Decode$decodeString, decoder, string));
				}));
	});
var $elm$http$Http$emptyBody = _Http_emptyBody;
var $elm$http$Http$Request = function (a) {
	return {$: 1, a: a};
};
var $elm$http$Http$State = F2(
	function (reqs, subs) {
		return {bx: reqs, bC: subs};
	});
var $elm$http$Http$init = $elm$core$Task$succeed(
	A2($elm$http$Http$State, $elm$core$Dict$empty, _List_Nil));
var $elm$core$Process$kill = _Scheduler_kill;
var $elm$core$Process$spawn = _Scheduler_spawn;
var $elm$http$Http$updateReqs = F3(
	function (router, cmds, reqs) {
		updateReqs:
		while (true) {
			if (!cmds.b) {
				return $elm$core$Task$succeed(reqs);
			} else {
				var cmd = cmds.a;
				var otherCmds = cmds.b;
				if (!cmd.$) {
					var tracker = cmd.a;
					var _v2 = A2($elm$core$Dict$get, tracker, reqs);
					if (_v2.$ === 1) {
						var $temp$router = router,
							$temp$cmds = otherCmds,
							$temp$reqs = reqs;
						router = $temp$router;
						cmds = $temp$cmds;
						reqs = $temp$reqs;
						continue updateReqs;
					} else {
						var pid = _v2.a;
						return A2(
							$elm$core$Task$andThen,
							function (_v3) {
								return A3(
									$elm$http$Http$updateReqs,
									router,
									otherCmds,
									A2($elm$core$Dict$remove, tracker, reqs));
							},
							$elm$core$Process$kill(pid));
					}
				} else {
					var req = cmd.a;
					return A2(
						$elm$core$Task$andThen,
						function (pid) {
							var _v4 = req.bG;
							if (_v4.$ === 1) {
								return A3($elm$http$Http$updateReqs, router, otherCmds, reqs);
							} else {
								var tracker = _v4.a;
								return A3(
									$elm$http$Http$updateReqs,
									router,
									otherCmds,
									A3($elm$core$Dict$insert, tracker, pid, reqs));
							}
						},
						$elm$core$Process$spawn(
							A3(
								_Http_toTask,
								router,
								$elm$core$Platform$sendToApp(router),
								req)));
				}
			}
		}
	});
var $elm$http$Http$onEffects = F4(
	function (router, cmds, subs, state) {
		return A2(
			$elm$core$Task$andThen,
			function (reqs) {
				return $elm$core$Task$succeed(
					A2($elm$http$Http$State, reqs, subs));
			},
			A3($elm$http$Http$updateReqs, router, cmds, state.bx));
	});
var $elm$core$List$maybeCons = F3(
	function (f, mx, xs) {
		var _v0 = f(mx);
		if (!_v0.$) {
			var x = _v0.a;
			return A2($elm$core$List$cons, x, xs);
		} else {
			return xs;
		}
	});
var $elm$core$List$filterMap = F2(
	function (f, xs) {
		return A3(
			$elm$core$List$foldr,
			$elm$core$List$maybeCons(f),
			_List_Nil,
			xs);
	});
var $elm$http$Http$maybeSend = F4(
	function (router, desiredTracker, progress, _v0) {
		var actualTracker = _v0.a;
		var toMsg = _v0.b;
		return _Utils_eq(desiredTracker, actualTracker) ? $elm$core$Maybe$Just(
			A2(
				$elm$core$Platform$sendToApp,
				router,
				toMsg(progress))) : $elm$core$Maybe$Nothing;
	});
var $elm$http$Http$onSelfMsg = F3(
	function (router, _v0, state) {
		var tracker = _v0.a;
		var progress = _v0.b;
		return A2(
			$elm$core$Task$andThen,
			function (_v1) {
				return $elm$core$Task$succeed(state);
			},
			$elm$core$Task$sequence(
				A2(
					$elm$core$List$filterMap,
					A3($elm$http$Http$maybeSend, router, tracker, progress),
					state.bC)));
	});
var $elm$http$Http$Cancel = function (a) {
	return {$: 0, a: a};
};
var $elm$http$Http$cmdMap = F2(
	function (func, cmd) {
		if (!cmd.$) {
			var tracker = cmd.a;
			return $elm$http$Http$Cancel(tracker);
		} else {
			var r = cmd.a;
			return $elm$http$Http$Request(
				{
					bK: r.bK,
					aG: r.aG,
					as: A2(_Http_mapExpect, func, r.as),
					bi: r.bi,
					bl: r.bl,
					bF: r.bF,
					bG: r.bG,
					aA: r.aA
				});
		}
	});
var $elm$http$Http$MySub = F2(
	function (a, b) {
		return {$: 0, a: a, b: b};
	});
var $elm$http$Http$subMap = F2(
	function (func, _v0) {
		var tracker = _v0.a;
		var toMsg = _v0.b;
		return A2(
			$elm$http$Http$MySub,
			tracker,
			A2($elm$core$Basics$composeR, toMsg, func));
	});
_Platform_effectManagers['Http'] = _Platform_createManager($elm$http$Http$init, $elm$http$Http$onEffects, $elm$http$Http$onSelfMsg, $elm$http$Http$cmdMap, $elm$http$Http$subMap);
var $elm$http$Http$command = _Platform_leaf('Http');
var $elm$http$Http$subscription = _Platform_leaf('Http');
var $elm$http$Http$request = function (r) {
	return $elm$http$Http$command(
		$elm$http$Http$Request(
			{bK: false, aG: r.aG, as: r.as, bi: r.bi, bl: r.bl, bF: r.bF, bG: r.bG, aA: r.aA}));
};
var $elm$http$Http$get = function (r) {
	return $elm$http$Http$request(
		{aG: $elm$http$Http$emptyBody, as: r.as, bi: _List_Nil, bl: 'GET', bF: $elm$core$Maybe$Nothing, bG: $elm$core$Maybe$Nothing, aA: r.aA});
};
var $elm$json$Json$Decode$list = _Json_decodeList;
var $author$project$Main$fetchPdfList = $elm$http$Http$get(
	{
		as: A2(
			$elm$http$Http$expectJson,
			$author$project$Main$GotFileList,
			A2(
				$elm$json$Json$Decode$field,
				'files',
				$elm$json$Json$Decode$list($author$project$Main$decodePdfFile))),
		aA: '/api/list_pdfs'
	});
var $elm$browser$Browser$Dom$getViewport = _Browser_withWindow(_Browser_getViewport);
var $author$project$Main$init = function (flags) {
	return _Utils_Tuple2(
		{e: 0, a_: flags.bH, ap: 900.0, aH: $elm$core$Dict$empty, O: $elm$core$Maybe$Nothing, Q: $elm$core$Maybe$Nothing, ae: $elm$core$Maybe$Nothing, R: $elm$core$Maybe$Nothing, F: $elm$core$Maybe$Nothing, n: _List_Nil, m: false, J: _List_Nil, aW: '900', af: 'NewHouse', aI: 'Rome', aJ: '0', aK: '12.0', S: false, y: 0, ah: 35.0, f: _List_Nil, _: 15.5, M: $elm$core$Maybe$Nothing, W: $elm$core$Maybe$Nothing, o: $author$project$Main$Idle, at: 10, I: 1, ab: 1, v: 1, aj: 210.0, aM: _List_Nil, s: 0, d: _List_Nil, x: false, aX: 42, av: '', C: $elm$core$Maybe$Nothing, A: $elm$core$Maybe$Nothing, k: $elm$core$Maybe$Nothing, D: '', aN: false, aO: true, aP: false, aw: true, aQ: true, ax: true, aR: 1.0, az: 60, c: _List_Nil, aZ: false, aE: 1.0},
		$elm$core$Platform$Cmd$batch(
			_List_fromArray(
				[
					$author$project$Main$fetchPdfList,
					A2($elm$core$Task$perform, $author$project$Main$GotViewport, $elm$browser$Browser$Dom$getViewport)
				])));
};
var $author$project$Main$ColorPickMove = F2(
	function (a, b) {
		return {$: 65, a: a, b: b};
	});
var $author$project$Main$EndColorPick = {$: 66};
var $elm$core$Platform$Sub$batch = _Platform_batch;
var $elm$json$Json$Decode$float = _Json_decodeFloat;
var $elm$browser$Browser$Events$Document = 0;
var $elm$browser$Browser$Events$MySub = F3(
	function (a, b, c) {
		return {$: 0, a: a, b: b, c: c};
	});
var $elm$browser$Browser$Events$State = F2(
	function (subs, pids) {
		return {bq: pids, bC: subs};
	});
var $elm$browser$Browser$Events$init = $elm$core$Task$succeed(
	A2($elm$browser$Browser$Events$State, _List_Nil, $elm$core$Dict$empty));
var $elm$browser$Browser$Events$nodeToKey = function (node) {
	if (!node) {
		return 'd_';
	} else {
		return 'w_';
	}
};
var $elm$browser$Browser$Events$addKey = function (sub) {
	var node = sub.a;
	var name = sub.b;
	return _Utils_Tuple2(
		_Utils_ap(
			$elm$browser$Browser$Events$nodeToKey(node),
			name),
		sub);
};
var $elm$core$Dict$fromList = function (assocs) {
	return A3(
		$elm$core$List$foldl,
		F2(
			function (_v0, dict) {
				var key = _v0.a;
				var value = _v0.b;
				return A3($elm$core$Dict$insert, key, value, dict);
			}),
		$elm$core$Dict$empty,
		assocs);
};
var $elm$core$Dict$foldl = F3(
	function (func, acc, dict) {
		foldl:
		while (true) {
			if (dict.$ === -2) {
				return acc;
			} else {
				var key = dict.b;
				var value = dict.c;
				var left = dict.d;
				var right = dict.e;
				var $temp$func = func,
					$temp$acc = A3(
					func,
					key,
					value,
					A3($elm$core$Dict$foldl, func, acc, left)),
					$temp$dict = right;
				func = $temp$func;
				acc = $temp$acc;
				dict = $temp$dict;
				continue foldl;
			}
		}
	});
var $elm$core$Dict$merge = F6(
	function (leftStep, bothStep, rightStep, leftDict, rightDict, initialResult) {
		var stepState = F3(
			function (rKey, rValue, _v0) {
				stepState:
				while (true) {
					var list = _v0.a;
					var result = _v0.b;
					if (!list.b) {
						return _Utils_Tuple2(
							list,
							A3(rightStep, rKey, rValue, result));
					} else {
						var _v2 = list.a;
						var lKey = _v2.a;
						var lValue = _v2.b;
						var rest = list.b;
						if (_Utils_cmp(lKey, rKey) < 0) {
							var $temp$rKey = rKey,
								$temp$rValue = rValue,
								$temp$_v0 = _Utils_Tuple2(
								rest,
								A3(leftStep, lKey, lValue, result));
							rKey = $temp$rKey;
							rValue = $temp$rValue;
							_v0 = $temp$_v0;
							continue stepState;
						} else {
							if (_Utils_cmp(lKey, rKey) > 0) {
								return _Utils_Tuple2(
									list,
									A3(rightStep, rKey, rValue, result));
							} else {
								return _Utils_Tuple2(
									rest,
									A4(bothStep, lKey, lValue, rValue, result));
							}
						}
					}
				}
			});
		var _v3 = A3(
			$elm$core$Dict$foldl,
			stepState,
			_Utils_Tuple2(
				$elm$core$Dict$toList(leftDict),
				initialResult),
			rightDict);
		var leftovers = _v3.a;
		var intermediateResult = _v3.b;
		return A3(
			$elm$core$List$foldl,
			F2(
				function (_v4, result) {
					var k = _v4.a;
					var v = _v4.b;
					return A3(leftStep, k, v, result);
				}),
			intermediateResult,
			leftovers);
	});
var $elm$browser$Browser$Events$Event = F2(
	function (key, event) {
		return {bg: event, bk: key};
	});
var $elm$browser$Browser$Events$spawn = F3(
	function (router, key, _v0) {
		var node = _v0.a;
		var name = _v0.b;
		var actualNode = function () {
			if (!node) {
				return _Browser_doc;
			} else {
				return _Browser_window;
			}
		}();
		return A2(
			$elm$core$Task$map,
			function (value) {
				return _Utils_Tuple2(key, value);
			},
			A3(
				_Browser_on,
				actualNode,
				name,
				function (event) {
					return A2(
						$elm$core$Platform$sendToSelf,
						router,
						A2($elm$browser$Browser$Events$Event, key, event));
				}));
	});
var $elm$core$Dict$union = F2(
	function (t1, t2) {
		return A3($elm$core$Dict$foldl, $elm$core$Dict$insert, t2, t1);
	});
var $elm$browser$Browser$Events$onEffects = F3(
	function (router, subs, state) {
		var stepRight = F3(
			function (key, sub, _v6) {
				var deads = _v6.a;
				var lives = _v6.b;
				var news = _v6.c;
				return _Utils_Tuple3(
					deads,
					lives,
					A2(
						$elm$core$List$cons,
						A3($elm$browser$Browser$Events$spawn, router, key, sub),
						news));
			});
		var stepLeft = F3(
			function (_v4, pid, _v5) {
				var deads = _v5.a;
				var lives = _v5.b;
				var news = _v5.c;
				return _Utils_Tuple3(
					A2($elm$core$List$cons, pid, deads),
					lives,
					news);
			});
		var stepBoth = F4(
			function (key, pid, _v2, _v3) {
				var deads = _v3.a;
				var lives = _v3.b;
				var news = _v3.c;
				return _Utils_Tuple3(
					deads,
					A3($elm$core$Dict$insert, key, pid, lives),
					news);
			});
		var newSubs = A2($elm$core$List$map, $elm$browser$Browser$Events$addKey, subs);
		var _v0 = A6(
			$elm$core$Dict$merge,
			stepLeft,
			stepBoth,
			stepRight,
			state.bq,
			$elm$core$Dict$fromList(newSubs),
			_Utils_Tuple3(_List_Nil, $elm$core$Dict$empty, _List_Nil));
		var deadPids = _v0.a;
		var livePids = _v0.b;
		var makeNewPids = _v0.c;
		return A2(
			$elm$core$Task$andThen,
			function (pids) {
				return $elm$core$Task$succeed(
					A2(
						$elm$browser$Browser$Events$State,
						newSubs,
						A2(
							$elm$core$Dict$union,
							livePids,
							$elm$core$Dict$fromList(pids))));
			},
			A2(
				$elm$core$Task$andThen,
				function (_v1) {
					return $elm$core$Task$sequence(makeNewPids);
				},
				$elm$core$Task$sequence(
					A2($elm$core$List$map, $elm$core$Process$kill, deadPids))));
	});
var $elm$browser$Browser$Events$onSelfMsg = F3(
	function (router, _v0, state) {
		var key = _v0.bk;
		var event = _v0.bg;
		var toMessage = function (_v2) {
			var subKey = _v2.a;
			var _v3 = _v2.b;
			var node = _v3.a;
			var name = _v3.b;
			var decoder = _v3.c;
			return _Utils_eq(subKey, key) ? A2(_Browser_decodeEvent, decoder, event) : $elm$core$Maybe$Nothing;
		};
		var messages = A2($elm$core$List$filterMap, toMessage, state.bC);
		return A2(
			$elm$core$Task$andThen,
			function (_v1) {
				return $elm$core$Task$succeed(state);
			},
			$elm$core$Task$sequence(
				A2(
					$elm$core$List$map,
					$elm$core$Platform$sendToApp(router),
					messages)));
	});
var $elm$browser$Browser$Events$subMap = F2(
	function (func, _v0) {
		var node = _v0.a;
		var name = _v0.b;
		var decoder = _v0.c;
		return A3(
			$elm$browser$Browser$Events$MySub,
			node,
			name,
			A2($elm$json$Json$Decode$map, func, decoder));
	});
_Platform_effectManagers['Browser.Events'] = _Platform_createManager($elm$browser$Browser$Events$init, $elm$browser$Browser$Events$onEffects, $elm$browser$Browser$Events$onSelfMsg, 0, $elm$browser$Browser$Events$subMap);
var $elm$browser$Browser$Events$subscription = _Platform_leaf('Browser.Events');
var $elm$browser$Browser$Events$on = F3(
	function (node, name, decoder) {
		return $elm$browser$Browser$Events$subscription(
			A3($elm$browser$Browser$Events$MySub, node, name, decoder));
	});
var $elm$browser$Browser$Events$onMouseMove = A2($elm$browser$Browser$Events$on, 0, 'mousemove');
var $elm$browser$Browser$Events$onMouseUp = A2($elm$browser$Browser$Events$on, 0, 'mouseup');
var $author$project$Main$subscriptions = function (model) {
	return $elm$core$Platform$Sub$batch(
		_Utils_ap(
			_List_Nil,
			function () {
				var _v0 = model.O;
				if (!_v0.$) {
					return _List_fromArray(
						[
							$elm$browser$Browser$Events$onMouseMove(
							A3(
								$elm$json$Json$Decode$map2,
								$author$project$Main$ColorPickMove,
								A2($elm$json$Json$Decode$field, 'clientX', $elm$json$Json$Decode$float),
								A2($elm$json$Json$Decode$field, 'clientY', $elm$json$Json$Decode$float))),
							$elm$browser$Browser$Events$onMouseUp(
							$elm$json$Json$Decode$succeed($author$project$Main$EndColorPick))
						]);
				} else {
					return _List_Nil;
				}
			}()));
};
var $author$project$Main$BrickRef = F5(
	function (id, x, y, width, height) {
		return {j: height, a: id, l: width, h: x, i: y};
	});
var $author$project$Main$Compositing = 1;
var $author$project$Main$FileSelected = function (a) {
	return {$: 2, a: a};
};
var $author$project$Main$Generated = 2;
var $author$project$Main$GotExportResponse = function (a) {
	return {$: 41, a: a};
};
var $author$project$Main$GridColorTarget = {$: 2};
var $author$project$Main$LoadError = function (a) {
	return {$: 3, a: a};
};
var $author$project$Main$Loaded = function (a) {
	return {$: 2, a: a};
};
var $author$project$Main$Loading = {$: 1};
var $author$project$Main$ModeGenerate = 1;
var $author$project$Main$ModeGroups = 4;
var $author$project$Main$ModePieces = 2;
var $author$project$Main$ModeWaves = 5;
var $author$project$Main$NoOp = {$: 68};
var $author$project$Main$OutlineColorTarget = {$: 3};
var $elm$core$Basics$negate = function (n) {
	return -n;
};
var $elm$core$Basics$abs = function (n) {
	return (n < 0) ? (-n) : n;
};
var $elm$core$List$any = F2(
	function (isOkay, list) {
		any:
		while (true) {
			if (!list.b) {
				return false;
			} else {
				var x = list.a;
				var xs = list.b;
				if (isOkay(x)) {
					return true;
				} else {
					var $temp$isOkay = isOkay,
						$temp$list = xs;
					isOkay = $temp$isOkay;
					list = $temp$list;
					continue any;
				}
			}
		}
	});
var $elm$core$Basics$composeL = F3(
	function (g, f, x) {
		return g(
			f(x));
	});
var $elm$core$Basics$not = _Basics_not;
var $elm$core$List$all = F2(
	function (isOkay, list) {
		return !A2(
			$elm$core$List$any,
			A2($elm$core$Basics$composeL, $elm$core$Basics$not, isOkay),
			list);
	});
var $elm$core$Task$onError = _Scheduler_onError;
var $elm$core$Task$attempt = F2(
	function (resultToMessage, task) {
		return $elm$core$Task$command(
			A2(
				$elm$core$Task$onError,
				A2(
					$elm$core$Basics$composeL,
					A2($elm$core$Basics$composeL, $elm$core$Task$succeed, resultToMessage),
					$elm$core$Result$Err),
				A2(
					$elm$core$Task$andThen,
					A2(
						$elm$core$Basics$composeL,
						A2($elm$core$Basics$composeL, $elm$core$Task$succeed, resultToMessage),
						$elm$core$Result$Ok),
					task)));
	});
var $elm$core$Basics$clamp = F3(
	function (low, high, number) {
		return (_Utils_cmp(number, low) < 0) ? low : ((_Utils_cmp(number, high) > 0) ? high : number);
	});
var $elm$core$List$append = F2(
	function (xs, ys) {
		if (!ys.b) {
			return xs;
		} else {
			return A3($elm$core$List$foldr, $elm$core$List$cons, ys, xs);
		}
	});
var $elm$core$List$concat = function (lists) {
	return A3($elm$core$List$foldr, $elm$core$List$append, _List_Nil, lists);
};
var $elm$core$List$concatMap = F2(
	function (f, list) {
		return $elm$core$List$concat(
			A2($elm$core$List$map, f, list));
	});
var $elm$core$Basics$modBy = _Basics_modBy;
var $author$project$Main$defaultHue = function (idx) {
	var _v0 = A2($elm$core$Basics$modBy, 7, idx);
	switch (_v0) {
		case 0:
			return 0;
		case 1:
			return 120;
		case 2:
			return 40;
		case 3:
			return 270;
		case 4:
			return 20;
		case 5:
			return 180;
		default:
			return 310;
	}
};
var $elm$core$List$drop = F2(
	function (n, list) {
		drop:
		while (true) {
			if (n <= 0) {
				return list;
			} else {
				if (!list.b) {
					return list;
				} else {
					var x = list.a;
					var xs = list.b;
					var $temp$n = n - 1,
						$temp$list = xs;
					n = $temp$n;
					list = $temp$list;
					continue drop;
				}
			}
		}
	});
var $elm$http$Http$expectBytesResponse = F2(
	function (toMsg, toResult) {
		return A3(
			_Http_expect,
			'arraybuffer',
			_Http_toDataView,
			A2($elm$core$Basics$composeR, toResult, toMsg));
	});
var $elm$http$Http$expectWhatever = function (toMsg) {
	return A2(
		$elm$http$Http$expectBytesResponse,
		toMsg,
		$elm$http$Http$resolve(
			function (_v0) {
				return $elm$core$Result$Ok(0);
			}));
};
var $elm$time$Time$Posix = $elm$core$Basics$identity;
var $elm$time$Time$millisToPosix = $elm$core$Basics$identity;
var $elm$file$File$Select$file = F2(
	function (mimes, toMsg) {
		return A2(
			$elm$core$Task$perform,
			toMsg,
			_File_uploadOne(mimes));
	});
var $elm$core$List$filter = F2(
	function (isGood, list) {
		return A3(
			$elm$core$List$foldr,
			F2(
				function (x, xs) {
					return isGood(x) ? A2($elm$core$List$cons, x, xs) : xs;
				}),
			_List_Nil,
			list);
	});
var $elm$json$Json$Encode$float = _Json_wrap;
var $elm$core$Basics$ge = _Utils_ge;
var $elm$browser$Browser$Dom$getViewportOf = _Browser_getViewportOf;
var $elm$core$List$head = function (list) {
	if (list.b) {
		var x = list.a;
		var xs = list.b;
		return $elm$core$Maybe$Just(x);
	} else {
		return $elm$core$Maybe$Nothing;
	}
};
var $author$project$Main$httpErrorToString = function (err) {
	switch (err.$) {
		case 0:
			var url = err.a;
			return 'Bad URL: ' + url;
		case 1:
			return 'Request timed out';
		case 2:
			return 'Network error';
		case 3:
			var code = err.a;
			return 'Server error: ' + $elm$core$String$fromInt(code);
		default:
			var m = err.a;
			return 'Bad response: ' + m;
	}
};
var $elm$json$Json$Encode$int = _Json_wrap;
var $elm$core$List$isEmpty = function (xs) {
	if (!xs.b) {
		return true;
	} else {
		return false;
	}
};
var $elm$http$Http$jsonBody = function (value) {
	return A2(
		_Http_pair,
		'application/json',
		A2($elm$json$Json$Encode$encode, 0, value));
};
var $elm$json$Json$Encode$list = F2(
	function (func, entries) {
		return _Json_wrap(
			A3(
				$elm$core$List$foldl,
				_Json_addEntry(func),
				_Json_emptyArray(0),
				entries));
	});
var $author$project$Main$GotLoadResponse = function (a) {
	return {$: 6, a: a};
};
var $author$project$Main$LoadResponse = function (canvas) {
	return function (bricks) {
		return function (hasComposite) {
			return function (hasBase) {
				return function (renderDpi) {
					return function (warnings) {
						return function (outlinesUrl) {
							return function (compositeUrl) {
								return function (blueprintBgUrl) {
									return function (lightsUrl) {
										return function (houseUnitsHigh) {
											return function (key) {
												return {bc: blueprintBgUrl, E: bricks, aq: canvas, a$: compositeUrl, bR: hasBase, a0: hasComposite, _: houseUnitsHigh, bk: key, a2: lightsUrl, bo: outlinesUrl, b_: renderDpi, aY: warnings};
											};
										};
									};
								};
							};
						};
					};
				};
			};
		};
	};
};
var $elm$json$Json$Decode$bool = _Json_decodeBool;
var $author$project$Main$Brick = F8(
	function (id, x, y, width, height, brickType, neighbors, polygon) {
		return {bM: brickType, j: height, a: id, bV: neighbors, z: polygon, l: width, h: x, i: y};
	});
var $elm$json$Json$Decode$index = _Json_decodeIndex;
var $elm$core$Tuple$pair = F2(
	function (a, b) {
		return _Utils_Tuple2(a, b);
	});
var $author$project$Main$decodePoint = A3(
	$elm$json$Json$Decode$map2,
	$elm$core$Tuple$pair,
	A2($elm$json$Json$Decode$index, 0, $elm$json$Json$Decode$float),
	A2($elm$json$Json$Decode$index, 1, $elm$json$Json$Decode$float));
var $elm$json$Json$Decode$map8 = _Json_map8;
var $author$project$Main$decodeBrick = A9(
	$elm$json$Json$Decode$map8,
	$author$project$Main$Brick,
	A2($elm$json$Json$Decode$field, 'id', $elm$json$Json$Decode$string),
	A2($elm$json$Json$Decode$field, 'x', $elm$json$Json$Decode$float),
	A2($elm$json$Json$Decode$field, 'y', $elm$json$Json$Decode$float),
	A2($elm$json$Json$Decode$field, 'width', $elm$json$Json$Decode$float),
	A2($elm$json$Json$Decode$field, 'height', $elm$json$Json$Decode$float),
	A2($elm$json$Json$Decode$field, 'type', $elm$json$Json$Decode$string),
	A2(
		$elm$json$Json$Decode$field,
		'neighbors',
		$elm$json$Json$Decode$list($elm$json$Json$Decode$string)),
	A2(
		$elm$json$Json$Decode$field,
		'polygon',
		$elm$json$Json$Decode$list($author$project$Main$decodePoint)));
var $author$project$Main$Canvas = F2(
	function (width, height) {
		return {j: height, l: width};
	});
var $author$project$Main$decodeCanvas = A3(
	$elm$json$Json$Decode$map2,
	$author$project$Main$Canvas,
	A2($elm$json$Json$Decode$field, 'width', $elm$json$Json$Decode$float),
	A2($elm$json$Json$Decode$field, 'height', $elm$json$Json$Decode$float));
var $elm$json$Json$Decode$oneOf = _Json_oneOf;
var $elm$json$Json$Decode$maybe = function (decoder) {
	return $elm$json$Json$Decode$oneOf(
		_List_fromArray(
			[
				A2($elm$json$Json$Decode$map, $elm$core$Maybe$Just, decoder),
				$elm$json$Json$Decode$succeed($elm$core$Maybe$Nothing)
			]));
};
var $elm$core$Maybe$withDefault = F2(
	function (_default, maybe) {
		if (!maybe.$) {
			var value = maybe.a;
			return value;
		} else {
			return _default;
		}
	});
var $author$project$Main$decodeLoadResponse = A2(
	$elm$json$Json$Decode$andThen,
	function (f) {
		return A2(
			$elm$json$Json$Decode$map,
			f,
			A2($elm$json$Json$Decode$field, 'key', $elm$json$Json$Decode$string));
	},
	A2(
		$elm$json$Json$Decode$andThen,
		function (f) {
			return A2(
				$elm$json$Json$Decode$map,
				f,
				A2(
					$elm$json$Json$Decode$map,
					$elm$core$Maybe$withDefault(15.5),
					$elm$json$Json$Decode$maybe(
						A2($elm$json$Json$Decode$field, 'houseUnitsHigh', $elm$json$Json$Decode$float))));
		},
		A2(
			$elm$json$Json$Decode$andThen,
			function (f) {
				return A2(
					$elm$json$Json$Decode$map,
					f,
					$elm$json$Json$Decode$maybe(
						A2($elm$json$Json$Decode$field, 'lights_url', $elm$json$Json$Decode$string)));
			},
			A2(
				$elm$json$Json$Decode$andThen,
				function (f) {
					return A2(
						$elm$json$Json$Decode$map,
						f,
						$elm$json$Json$Decode$maybe(
							A2($elm$json$Json$Decode$field, 'blueprint_bg_url', $elm$json$Json$Decode$string)));
				},
				A9(
					$elm$json$Json$Decode$map8,
					F8(
						function (canvas, bricks, hasComposite, hasBase, renderDpi, warnings, outlinesUrl, compositeUrl) {
							return F2(
								function (blueprintBgUrl, lightsUrl) {
									return F2(
										function (houseUnitsHigh, key) {
											return $author$project$Main$LoadResponse(canvas)(bricks)(hasComposite)(hasBase)(renderDpi)(warnings)(outlinesUrl)(compositeUrl)(blueprintBgUrl)(lightsUrl)(houseUnitsHigh)(key);
										});
								});
						}),
					A2($elm$json$Json$Decode$field, 'canvas', $author$project$Main$decodeCanvas),
					A2(
						$elm$json$Json$Decode$field,
						'bricks',
						$elm$json$Json$Decode$list($author$project$Main$decodeBrick)),
					A2($elm$json$Json$Decode$field, 'has_composite', $elm$json$Json$Decode$bool),
					A2($elm$json$Json$Decode$field, 'has_base', $elm$json$Json$Decode$bool),
					A2($elm$json$Json$Decode$field, 'render_dpi', $elm$json$Json$Decode$float),
					A2(
						$elm$json$Json$Decode$field,
						'warnings',
						$elm$json$Json$Decode$list($elm$json$Json$Decode$string)),
					A2(
						$elm$json$Json$Decode$map,
						$elm$core$Maybe$withDefault('/api/outlines.png'),
						$elm$json$Json$Decode$maybe(
							A2($elm$json$Json$Decode$field, 'outlines_url', $elm$json$Json$Decode$string))),
					A2(
						$elm$json$Json$Decode$map,
						$elm$core$Maybe$withDefault('/api/composite.png'),
						$elm$json$Json$Decode$maybe(
							A2($elm$json$Json$Decode$field, 'composite_url', $elm$json$Json$Decode$string))))))));
var $elm$json$Json$Encode$object = function (pairs) {
	return _Json_wrap(
		A3(
			$elm$core$List$foldl,
			F2(
				function (_v0, obj) {
					var k = _v0.a;
					var v = _v0.b;
					return A3(_Json_addField, k, v, obj);
				}),
			_Json_emptyObject(0),
			pairs));
};
var $elm$http$Http$riskyRequest = function (r) {
	return $elm$http$Http$command(
		$elm$http$Http$Request(
			{bK: true, aG: r.aG, as: r.as, bi: r.bi, bl: r.bl, bF: r.bF, bG: r.bG, aA: r.aA}));
};
var $elm$core$Basics$round = _Basics_round;
var $elm$json$Json$Encode$string = _Json_wrap;
var $author$project$Main$loadPdf = F3(
	function (key, path, canvasHeight) {
		return $elm$http$Http$riskyRequest(
			{
				aG: $elm$http$Http$jsonBody(
					$elm$json$Json$Encode$object(
						_List_fromArray(
							[
								_Utils_Tuple2(
								'path',
								$elm$json$Json$Encode$string(path)),
								_Utils_Tuple2(
								'canvas_height',
								$elm$json$Json$Encode$int(
									$elm$core$Basics$round(canvasHeight)))
							]))),
				as: A2($elm$http$Http$expectJson, $author$project$Main$GotLoadResponse, $author$project$Main$decodeLoadResponse),
				bi: _List_Nil,
				bl: 'POST',
				bF: $elm$core$Maybe$Just((5 * 60) * 1000),
				bG: $elm$core$Maybe$Nothing,
				aA: '/api/s/' + (key + '/load')
			});
	});
var $author$project$Main$logBrick = _Platform_outgoingPort('logBrick', $elm$core$Basics$identity);
var $elm$core$Maybe$map = F2(
	function (f, maybe) {
		if (!maybe.$) {
			var value = maybe.a;
			return $elm$core$Maybe$Just(
				f(value));
		} else {
			return $elm$core$Maybe$Nothing;
		}
	});
var $elm$core$List$member = F2(
	function (x, xs) {
		return A2(
			$elm$core$List$any,
			function (a) {
				return _Utils_eq(a, x);
			},
			xs);
	});
var $author$project$Main$GotMergeResponse = function (a) {
	return {$: 11, a: a};
};
var $author$project$Main$MergeResponse = function (pieces) {
	return {d: pieces};
};
var $elm$json$Json$Decode$map5 = _Json_map5;
var $author$project$Main$decodeBrickRef = A6(
	$elm$json$Json$Decode$map5,
	$author$project$Main$BrickRef,
	A2($elm$json$Json$Decode$field, 'id', $elm$json$Json$Decode$string),
	A2($elm$json$Json$Decode$field, 'x', $elm$json$Json$Decode$float),
	A2($elm$json$Json$Decode$field, 'y', $elm$json$Json$Decode$float),
	A2($elm$json$Json$Decode$field, 'width', $elm$json$Json$Decode$float),
	A2($elm$json$Json$Decode$field, 'height', $elm$json$Json$Decode$float));
var $author$project$Main$decodePiece = A9(
	$elm$json$Json$Decode$map8,
	F8(
		function (id_, x_, y_, w_, h_, brickIds_, bricks_, polygon_) {
			return {u: brickIds_, E: bricks_, j: h_, a: id_, G: '', au: '', z: polygon_, l: w_, h: x_, i: y_};
		}),
	A2($elm$json$Json$Decode$field, 'id', $elm$json$Json$Decode$string),
	A2($elm$json$Json$Decode$field, 'x', $elm$json$Json$Decode$float),
	A2($elm$json$Json$Decode$field, 'y', $elm$json$Json$Decode$float),
	A2($elm$json$Json$Decode$field, 'width', $elm$json$Json$Decode$float),
	A2($elm$json$Json$Decode$field, 'height', $elm$json$Json$Decode$float),
	A2(
		$elm$json$Json$Decode$field,
		'brick_ids',
		$elm$json$Json$Decode$list($elm$json$Json$Decode$string)),
	A2(
		$elm$json$Json$Decode$field,
		'bricks',
		$elm$json$Json$Decode$list($author$project$Main$decodeBrickRef)),
	A2(
		$elm$json$Json$Decode$field,
		'polygon',
		$elm$json$Json$Decode$list($author$project$Main$decodePoint)));
var $author$project$Main$decodeMergeResponse = A2(
	$elm$json$Json$Decode$map,
	$author$project$Main$MergeResponse,
	A2(
		$elm$json$Json$Decode$field,
		'pieces',
		$elm$json$Json$Decode$list($author$project$Main$decodePiece)));
var $elm$http$Http$post = function (r) {
	return $elm$http$Http$request(
		{aG: r.aG, as: r.as, bi: _List_Nil, bl: 'POST', bF: $elm$core$Maybe$Nothing, bG: $elm$core$Maybe$Nothing, aA: r.aA});
};
var $author$project$Main$mergeBricks = F4(
	function (key, targetCount, minBorder, seed) {
		return $elm$http$Http$post(
			{
				aG: $elm$http$Http$jsonBody(
					$elm$json$Json$Encode$object(
						_List_fromArray(
							[
								_Utils_Tuple2(
								'target_count',
								$elm$json$Json$Encode$int(targetCount)),
								_Utils_Tuple2(
								'seed',
								$elm$json$Json$Encode$int(seed)),
								_Utils_Tuple2(
								'min_border',
								$elm$json$Json$Encode$int(minBorder))
							]))),
				as: A2($elm$http$Http$expectJson, $author$project$Main$GotMergeResponse, $author$project$Main$decodeMergeResponse),
				aA: '/api/s/' + (key + '/merge')
			});
	});
var $elm$core$Basics$min = F2(
	function (x, y) {
		return (_Utils_cmp(x, y) < 0) ? x : y;
	});
var $elm$file$File$name = _File_name;
var $elm$core$Basics$neq = _Utils_notEqual;
var $elm$core$Platform$Cmd$none = $elm$core$Platform$Cmd$batch(_List_Nil);
var $elm$json$Json$Encode$null = _Json_encodeNull;
var $elm$core$List$maximum = function (list) {
	if (list.b) {
		var x = list.a;
		var xs = list.b;
		return $elm$core$Maybe$Just(
			A3($elm$core$List$foldl, $elm$core$Basics$max, x, xs));
	} else {
		return $elm$core$Maybe$Nothing;
	}
};
var $elm$core$List$minimum = function (list) {
	if (list.b) {
		var x = list.a;
		var xs = list.b;
		return $elm$core$Maybe$Just(
			A3($elm$core$List$foldl, $elm$core$Basics$min, x, xs));
	} else {
		return $elm$core$Maybe$Nothing;
	}
};
var $author$project$Main$recalcPieceBbox = F3(
	function (sessionKey, bricksById, piece) {
		var bricks = A2(
			$elm$core$List$filterMap,
			function (bid) {
				return A2($elm$core$Dict$get, bid, bricksById);
			},
			piece.u);
		var newBrickRefs = A2(
			$elm$core$List$map,
			function (b) {
				return A5($author$project$Main$BrickRef, b.a, b.h, b.i, b.l, b.j);
			},
			bricks);
		var x2s = A2(
			$elm$core$List$map,
			function (b) {
				return b.h + b.l;
			},
			bricks);
		var xs = A2(
			$elm$core$List$map,
			function ($) {
				return $.h;
			},
			bricks);
		var y2s = A2(
			$elm$core$List$map,
			function (b) {
				return b.i + b.j;
			},
			bricks);
		var ys = A2(
			$elm$core$List$map,
			function ($) {
				return $.i;
			},
			bricks);
		var _v0 = $elm$core$List$minimum(xs);
		if (_v0.$ === 1) {
			return piece;
		} else {
			var x = _v0.a;
			var _v1 = _Utils_Tuple3(
				$elm$core$List$minimum(ys),
				$elm$core$List$maximum(x2s),
				$elm$core$List$maximum(y2s));
			if (((!_v1.a.$) && (!_v1.b.$)) && (!_v1.c.$)) {
				var y = _v1.a.a;
				var x2 = _v1.b.a;
				var y2 = _v1.c.a;
				return _Utils_update(
					piece,
					{E: newBrickRefs, j: y2 - y, G: '/api/s/' + (sessionKey + ('/piece/' + (piece.a + '.png'))), au: '/api/s/' + (sessionKey + ('/piece_outline/' + (piece.a + '.png'))), z: _List_Nil, l: x2 - x, h: x, i: y});
			} else {
				return piece;
			}
		}
	});
var $author$project$Main$GotPiecePolygons = function (a) {
	return {$: 34, a: a};
};
var $author$project$Main$decodePiecePolygonResponse = A2(
	$elm$json$Json$Decode$field,
	'pieces',
	$elm$json$Json$Decode$list(
		A3(
			$elm$json$Json$Decode$map2,
			$elm$core$Tuple$pair,
			A2($elm$json$Json$Decode$field, 'id', $elm$json$Json$Decode$string),
			A2(
				$elm$json$Json$Decode$field,
				'polygon',
				$elm$json$Json$Decode$list($author$project$Main$decodePoint)))));
var $author$project$Main$recomputePiecePolygons = F2(
	function (key, pieces) {
		return $elm$http$Http$post(
			{
				aG: $elm$http$Http$jsonBody(
					$elm$json$Json$Encode$object(
						_List_fromArray(
							[
								_Utils_Tuple2(
								'pieces',
								A2(
									$elm$json$Json$Encode$list,
									function (p) {
										return $elm$json$Json$Encode$object(
											_List_fromArray(
												[
													_Utils_Tuple2(
													'id',
													$elm$json$Json$Encode$string(p.a)),
													_Utils_Tuple2(
													'brick_ids',
													A2($elm$json$Json$Encode$list, $elm$json$Json$Encode$string, p.u))
												]));
									},
									pieces))
							]))),
				as: A2($elm$http$Http$expectJson, $author$project$Main$GotPiecePolygons, $author$project$Main$decodePiecePolygonResponse),
				aA: '/api/s/' + (key + '/merge')
			});
	});
var $elm$core$String$replace = F3(
	function (before, after, string) {
		return A2(
			$elm$core$String$join,
			after,
			A2($elm$core$String$split, before, string));
	});
var $elm$browser$Browser$Dom$setViewportOf = _Browser_setViewportOf;
var $elm$core$Process$sleep = _Process_sleep;
var $author$project$Main$scrollToBottom = A2(
	$elm$core$Task$attempt,
	function (_v0) {
		return $author$project$Main$NoOp;
	},
	A2(
		$elm$core$Task$andThen,
		function (_v1) {
			return A3($elm$browser$Browser$Dom$setViewportOf, 'house-scroll', 0, 999999);
		},
		$elm$core$Process$sleep(0)));
var $author$project$Main$scrollTrayToEnd = A2(
	$elm$core$Task$attempt,
	function (_v0) {
		return $author$project$Main$NoOp;
	},
	A2(
		$elm$core$Task$andThen,
		function (_v1) {
			return A3($elm$browser$Browser$Dom$setViewportOf, 'wave-tray-scroll', 999999, 0);
		},
		$elm$core$Process$sleep(0)));
var $author$project$Main$setTitle = _Platform_outgoingPort('setTitle', $elm$json$Json$Encode$string);
var $elm$core$List$takeReverse = F3(
	function (n, list, kept) {
		takeReverse:
		while (true) {
			if (n <= 0) {
				return kept;
			} else {
				if (!list.b) {
					return kept;
				} else {
					var x = list.a;
					var xs = list.b;
					var $temp$n = n - 1,
						$temp$list = xs,
						$temp$kept = A2($elm$core$List$cons, x, kept);
					n = $temp$n;
					list = $temp$list;
					kept = $temp$kept;
					continue takeReverse;
				}
			}
		}
	});
var $elm$core$List$takeTailRec = F2(
	function (n, list) {
		return $elm$core$List$reverse(
			A3($elm$core$List$takeReverse, n, list, _List_Nil));
	});
var $elm$core$List$takeFast = F3(
	function (ctr, n, list) {
		if (n <= 0) {
			return _List_Nil;
		} else {
			var _v0 = _Utils_Tuple2(n, list);
			_v0$1:
			while (true) {
				_v0$5:
				while (true) {
					if (!_v0.b.b) {
						return list;
					} else {
						if (_v0.b.b.b) {
							switch (_v0.a) {
								case 1:
									break _v0$1;
								case 2:
									var _v2 = _v0.b;
									var x = _v2.a;
									var _v3 = _v2.b;
									var y = _v3.a;
									return _List_fromArray(
										[x, y]);
								case 3:
									if (_v0.b.b.b.b) {
										var _v4 = _v0.b;
										var x = _v4.a;
										var _v5 = _v4.b;
										var y = _v5.a;
										var _v6 = _v5.b;
										var z = _v6.a;
										return _List_fromArray(
											[x, y, z]);
									} else {
										break _v0$5;
									}
								default:
									if (_v0.b.b.b.b && _v0.b.b.b.b.b) {
										var _v7 = _v0.b;
										var x = _v7.a;
										var _v8 = _v7.b;
										var y = _v8.a;
										var _v9 = _v8.b;
										var z = _v9.a;
										var _v10 = _v9.b;
										var w = _v10.a;
										var tl = _v10.b;
										return (ctr > 1000) ? A2(
											$elm$core$List$cons,
											x,
											A2(
												$elm$core$List$cons,
												y,
												A2(
													$elm$core$List$cons,
													z,
													A2(
														$elm$core$List$cons,
														w,
														A2($elm$core$List$takeTailRec, n - 4, tl))))) : A2(
											$elm$core$List$cons,
											x,
											A2(
												$elm$core$List$cons,
												y,
												A2(
													$elm$core$List$cons,
													z,
													A2(
														$elm$core$List$cons,
														w,
														A3($elm$core$List$takeFast, ctr + 1, n - 4, tl)))));
									} else {
										break _v0$5;
									}
							}
						} else {
							if (_v0.a === 1) {
								break _v0$1;
							} else {
								break _v0$5;
							}
						}
					}
				}
				return list;
			}
			var _v1 = _v0.b;
			var x = _v1.a;
			return _List_fromArray(
				[x]);
		}
	});
var $elm$core$List$take = F2(
	function (n, list) {
		return A3($elm$core$List$takeFast, 0, n, list);
	});
var $elm$core$String$toFloat = _String_toFloat;
var $author$project$Main$FileUploaded = function (a) {
	return {$: 3, a: a};
};
var $elm$http$Http$filePart = _Http_pair;
var $elm$http$Http$multipartBody = function (parts) {
	return A2(
		_Http_pair,
		'',
		_Http_toFormData(parts));
};
var $author$project$Main$uploadFile = function (file) {
	return $elm$http$Http$post(
		{
			aG: $elm$http$Http$multipartBody(
				_List_fromArray(
					[
						A2($elm$http$Http$filePart, 'file', file)
					])),
			as: A2(
				$elm$http$Http$expectJson,
				$author$project$Main$FileUploaded,
				A2($elm$json$Json$Decode$field, 'path', $elm$json$Json$Decode$string)),
			aA: '/api/upload_file'
		});
};
var $author$project$Main$withPieceUrls = F2(
	function (key, p) {
		return _Utils_update(
			p,
			{G: '/api/s/' + (key + ('/piece/' + (p.a + '.png'))), au: '/api/s/' + (key + ('/piece_outline/' + (p.a + '.png')))});
	});
var $author$project$Main$update = F2(
	function (msg, model) {
		switch (msg.$) {
			case 0:
				if (!msg.a.$) {
					var files = msg.a.a;
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{aM: files}),
						$elm$core$Platform$Cmd$none);
				} else {
					return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
				}
			case 1:
				return _Utils_Tuple2(
					model,
					A2(
						$elm$file$File$Select$file,
						_List_fromArray(
							['.pdf', 'application/pdf', '.ai', 'application/illustrator']),
						$author$project$Main$FileSelected));
			case 2:
				var file = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							e: 0,
							n: _List_Nil,
							m: false,
							J: _List_Nil,
							y: 0,
							o: $author$project$Main$Loading,
							v: 1,
							s: 0,
							d: _List_Nil,
							x: false,
							av: $elm$file$File$name(file),
							A: $elm$core$Maybe$Nothing,
							k: $elm$core$Maybe$Nothing,
							c: _List_Nil
						}),
					$author$project$Main$uploadFile(file));
			case 3:
				if (!msg.a.$) {
					var path = msg.a.a;
					var key = $elm$core$String$fromInt(model.ab);
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{ab: model.ab + 1, D: key}),
						A3($author$project$Main$loadPdf, key, path, model.ap));
				} else {
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{o: $author$project$Main$Idle}),
						$elm$core$Platform$Cmd$none);
				}
			case 5:
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{e: 0, n: _List_Nil, m: false, J: _List_Nil, y: 0, o: $author$project$Main$Idle, v: 1, s: 0, d: _List_Nil, x: false, av: '', A: $elm$core$Maybe$Nothing, k: $elm$core$Maybe$Nothing, D: '', c: _List_Nil}),
					$author$project$Main$fetchPdfList);
			case 4:
				var path = msg.a;
				var key = $elm$core$String$fromInt(model.ab);
				var baseName = A3(
					$elm$core$String$replace,
					'.pdf',
					'',
					A3(
						$elm$core$String$replace,
						'.ai',
						'',
						A2(
							$elm$core$Maybe$withDefault,
							path,
							$elm$core$List$head(
								$elm$core$List$reverse(
									A2($elm$core$String$split, '/', path))))));
				var houseName = A2($elm$core$String$startsWith, '_', baseName) ? A2($elm$core$String$dropLeft, 1, baseName) : baseName;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{e: 0, n: _List_Nil, m: false, J: _List_Nil, af: houseName, y: 0, o: $author$project$Main$Loading, ab: model.ab + 1, v: 1, s: 0, d: _List_Nil, x: false, av: path, A: $elm$core$Maybe$Nothing, k: $elm$core$Maybe$Nothing, D: key, c: _List_Nil}),
					A3($author$project$Main$loadPdf, key, path, model.ap));
			case 6:
				if (!msg.a.$) {
					var response = msg.a.a;
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{
								e: 1,
								aH: $elm$core$Dict$fromList(
									A2(
										$elm$core$List$map,
										function (b) {
											return _Utils_Tuple2(b.a, b);
										},
										response.E)),
								y: 0,
								f: _List_Nil,
								_: response._,
								o: $author$project$Main$Loaded(response),
								I: 1,
								v: 1,
								s: 0,
								d: _List_Nil,
								C: $elm$core$Maybe$Nothing,
								A: $elm$core$Maybe$Nothing,
								k: $elm$core$Maybe$Nothing,
								D: response.bk,
								c: _List_Nil
							}),
						$author$project$Main$setTitle(model.af + ' — House Puzzle Editor'));
				} else {
					var err = msg.a.a;
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{
								o: $author$project$Main$LoadError(
									$author$project$Main$httpErrorToString(err))
							}),
						$elm$core$Platform$Cmd$none);
				}
			case 7:
				var s = msg.a;
				var _v1 = $elm$core$String$toInt(s);
				if (!_v1.$) {
					var n = _v1.a;
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{
								az: A2($elm$core$Basics$max, 1, n)
							}),
						$elm$core$Platform$Cmd$none);
				} else {
					return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
				}
			case 8:
				var s = msg.a;
				var _v2 = $elm$core$String$toInt(s);
				if (!_v2.$) {
					var n = _v2.a;
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{
								at: A2($elm$core$Basics$max, 0, n)
							}),
						$elm$core$Platform$Cmd$none);
				} else {
					return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
				}
			case 9:
				var s = msg.a;
				var _v3 = $elm$core$String$toInt(s);
				if (!_v3.$) {
					var n = _v3.a;
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{
								aX: A2($elm$core$Basics$max, 0, n)
							}),
						$elm$core$Platform$Cmd$none);
				} else {
					return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
				}
			case 10:
				var _v4 = model.o;
				if (_v4.$ === 2) {
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{n: _List_Nil, m: false, J: _List_Nil, y: 1, v: 1, d: _List_Nil, x: false, A: $elm$core$Maybe$Nothing, k: $elm$core$Maybe$Nothing, c: _List_Nil}),
						A4($author$project$Main$mergeBricks, model.D, model.az, model.at, model.aX));
				} else {
					return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
				}
			case 11:
				if (!msg.a.$) {
					var response = msg.a.a;
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{
								e: 2,
								y: 2,
								s: model.s + 1,
								d: A2(
									$elm$core$List$map,
									$author$project$Main$withPieceUrls(model.D),
									response.d),
								x: false
							}),
						A2($elm$core$Task$perform, $author$project$Main$GotViewport, $elm$browser$Browser$Dom$getViewport));
				} else {
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{y: 0, x: false}),
						$elm$core$Platform$Cmd$none);
				}
			case 13:
				var mode = msg.a;
				var recomputeViewport = A2($elm$core$Task$perform, $author$project$Main$GotViewport, $elm$browser$Browser$Dom$getViewport);
				var baseModel = _Utils_update(
					model,
					{e: mode, n: _List_Nil, m: false, J: _List_Nil});
				if (mode === 5) {
					var _v5 = model.c;
					if (!_v5.b) {
						var newWave = {
							r: $author$project$Main$defaultHue(model.v - 1),
							a: model.v,
							g: false,
							N: 'Wave ' + $elm$core$String$fromInt(model.v),
							ai: 0.3,
							b: _List_Nil,
							V: true
						};
						return _Utils_Tuple2(
							_Utils_update(
								baseModel,
								{
									v: model.v + 1,
									k: $elm$core$Maybe$Just(newWave.a),
									c: _List_fromArray(
										[newWave])
								}),
							recomputeViewport);
					} else {
						var first = _v5.a;
						return _Utils_Tuple2(
							_Utils_update(
								baseModel,
								{
									k: _Utils_eq(baseModel.k, $elm$core$Maybe$Nothing) ? $elm$core$Maybe$Just(first.a) : baseModel.k
								}),
							recomputeViewport);
					}
				} else {
					if (mode === 4) {
						var _v6 = model.f;
						if (!_v6.b) {
							var newGroup = {
								r: $author$project$Main$defaultHue(model.I - 1),
								a: model.I,
								g: false,
								N: 'Group ' + $elm$core$String$fromInt(model.I),
								b: _List_Nil
							};
							return _Utils_Tuple2(
								_Utils_update(
									baseModel,
									{
										f: _List_fromArray(
											[newGroup]),
										I: model.I + 1,
										C: $elm$core$Maybe$Just(newGroup.a)
									}),
								recomputeViewport);
						} else {
							var first = _v6.a;
							return _Utils_Tuple2(
								_Utils_update(
									baseModel,
									{
										C: _Utils_eq(baseModel.C, $elm$core$Maybe$Nothing) ? $elm$core$Maybe$Just(first.a) : baseModel.C
									}),
								recomputeViewport);
						}
					} else {
						return _Utils_Tuple2(baseModel, recomputeViewport);
					}
				}
			case 14:
				var checked = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{aQ: checked}),
					$elm$core$Platform$Cmd$none);
			case 15:
				var checked = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{aN: checked}),
					$elm$core$Platform$Cmd$none);
			case 16:
				var checked = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{aw: checked}),
					$elm$core$Platform$Cmd$none);
			case 17:
				var checked = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{aP: checked}),
					$elm$core$Platform$Cmd$none);
			case 19:
				var checked = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{aO: checked}),
					$elm$core$Platform$Cmd$none);
			case 20:
				var checked = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{ax: checked}),
					$elm$core$Platform$Cmd$none);
			case 21:
				var newWave = {
					r: $author$project$Main$defaultHue(model.v - 1),
					a: model.v,
					g: false,
					N: 'Wave ' + $elm$core$String$fromInt(model.v),
					ai: 0.3,
					b: _List_Nil,
					V: true
				};
				var lockedWaves = A2(
					$elm$core$List$map,
					function (w) {
						return _Utils_eq(
							$elm$core$Maybe$Just(w.a),
							model.k) ? _Utils_update(
							w,
							{g: true}) : w;
					},
					model.c);
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							v: model.v + 1,
							k: $elm$core$Maybe$Just(newWave.a),
							c: _Utils_ap(
								_List_fromArray(
									[newWave]),
								lockedWaves)
						}),
					$elm$core$Platform$Cmd$none);
			case 22:
				var waveId = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							c: A2(
								$elm$core$List$map,
								function (w) {
									return _Utils_eq(w.a, waveId) ? _Utils_update(
										w,
										{V: !w.V}) : w;
								},
								model.c)
						}),
					$elm$core$Platform$Cmd$none);
			case 23:
				var mid = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{M: mid}),
					$elm$core$Platform$Cmd$none);
			case 24:
				var pid = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							A: _Utils_eq(
								model.A,
								$elm$core$Maybe$Just(pid)) ? $elm$core$Maybe$Nothing : $elm$core$Maybe$Just(pid)
						}),
					$elm$core$Platform$Cmd$none);
			case 25:
				var mwid = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{k: mwid}),
					$elm$core$Platform$Cmd$none);
			case 26:
				var pid = msg.a;
				var _v7 = model.k;
				if (_v7.$ === 1) {
					return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
				} else {
					var wid = _v7.a;
					var targetWave = $elm$core$List$head(
						A2(
							$elm$core$List$filter,
							function (w) {
								return _Utils_eq(w.a, wid);
							},
							model.c));
					var targetLocked = A2(
						$elm$core$Maybe$withDefault,
						false,
						A2(
							$elm$core$Maybe$map,
							function ($) {
								return $.g;
							},
							targetWave));
					var sourceLocked = A2(
						$elm$core$List$any,
						function (w) {
							return w.g && A2($elm$core$List$member, pid, w.b);
						},
						model.c);
					var alreadyIn = A2(
						$elm$core$Maybe$withDefault,
						false,
						A2(
							$elm$core$Maybe$map,
							function (w) {
								return A2($elm$core$List$member, pid, w.b);
							},
							targetWave));
					var didAdd = (!targetLocked) && ((!alreadyIn) && (!sourceLocked));
					var updatedWaves = (targetLocked || ((!alreadyIn) && sourceLocked)) ? model.c : A2(
						$elm$core$List$map,
						function (w) {
							return _Utils_eq(w.a, wid) ? (alreadyIn ? _Utils_update(
								w,
								{
									b: A2(
										$elm$core$List$filter,
										function (p) {
											return !_Utils_eq(p, pid);
										},
										w.b)
								}) : _Utils_update(
								w,
								{
									b: _Utils_ap(
										w.b,
										_List_fromArray(
											[pid]))
								})) : ((!alreadyIn) ? _Utils_update(
								w,
								{
									b: A2(
										$elm$core$List$filter,
										function (p) {
											return !_Utils_eq(p, pid);
										},
										w.b)
								}) : w);
						},
						model.c);
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{c: updatedWaves}),
						didAdd ? $author$project$Main$scrollTrayToEnd : $elm$core$Platform$Cmd$none);
				}
			case 27:
				var wid = msg.a;
				var pid = msg.b;
				var waveLocked = A2(
					$elm$core$List$any,
					function (w) {
						return _Utils_eq(w.a, wid) && w.g;
					},
					model.c);
				return waveLocked ? _Utils_Tuple2(model, $elm$core$Platform$Cmd$none) : _Utils_Tuple2(
					_Utils_update(
						model,
						{
							c: A2(
								$elm$core$List$map,
								function (w) {
									return _Utils_eq(w.a, wid) ? _Utils_update(
										w,
										{
											b: A2(
												$elm$core$List$filter,
												function (p) {
													return !_Utils_eq(p, pid);
												},
												w.b)
										}) : w;
								},
								model.c)
						}),
					$elm$core$Platform$Cmd$none);
			case 28:
				var wid = msg.a;
				var dir = msg.b;
				var indexed = A2($elm$core$List$indexedMap, $elm$core$Tuple$pair, model.c);
				var maybeIdx = A2(
					$elm$core$Maybe$map,
					$elm$core$Tuple$first,
					$elm$core$List$head(
						A2(
							$elm$core$List$filter,
							function (_v9) {
								var w = _v9.b;
								return _Utils_eq(w.a, wid);
							},
							indexed)));
				var swapped = function () {
					if (maybeIdx.$ === 1) {
						return model.c;
					} else {
						var i = maybeIdx.a;
						var n = $elm$core$List$length(model.c);
						var j = i + dir;
						return ((j < 0) || (_Utils_cmp(j, n) > -1)) ? model.c : A2(
							$elm$core$List$indexedMap,
							F2(
								function (k, w) {
									return _Utils_eq(k, i) ? A2(
										$elm$core$Maybe$withDefault,
										w,
										$elm$core$List$head(
											A2($elm$core$List$drop, j, model.c))) : (_Utils_eq(k, j) ? A2(
										$elm$core$Maybe$withDefault,
										w,
										$elm$core$List$head(
											A2($elm$core$List$drop, i, model.c))) : w);
								}),
							model.c);
					}
				}();
				var renumbered = A2(
					$elm$core$List$indexedMap,
					F2(
						function (i, w) {
							return _Utils_update(
								w,
								{
									N: 'Wave ' + $elm$core$String$fromInt(i + 1)
								});
						}),
					swapped);
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{c: renumbered}),
					$elm$core$Platform$Cmd$none);
			case 29:
				var wid = msg.a;
				var newSelectedWaveId = _Utils_eq(
					model.k,
					$elm$core$Maybe$Just(wid)) ? $elm$core$Maybe$Nothing : model.k;
				var filtered = A2(
					$elm$core$List$filter,
					function (w) {
						return !_Utils_eq(w.a, wid);
					},
					model.c);
				var renumbered = A2(
					$elm$core$List$indexedMap,
					F2(
						function (i, w) {
							return _Utils_update(
								w,
								{
									N: 'Wave ' + $elm$core$String$fromInt(i + 1)
								});
						}),
					filtered);
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{k: newSelectedWaveId, c: renumbered}),
					$elm$core$Platform$Cmd$none);
			case 30:
				var _v10 = model.A;
				if (_v10.$ === 1) {
					return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
				} else {
					var pid = _v10.a;
					var _v11 = $elm$core$List$head(
						A2(
							$elm$core$List$filter,
							function (p) {
								return _Utils_eq(p.a, pid);
							},
							model.d));
					if (_v11.$ === 1) {
						return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
					} else {
						var piece = _v11.a;
						return _Utils_Tuple2(
							_Utils_update(
								model,
								{n: piece.u, m: true, J: piece.u}),
							$elm$core$Platform$Cmd$none);
					}
				}
			case 31:
				var bid = msg.a;
				var newList = A2($elm$core$List$member, bid, model.n) ? (($elm$core$List$length(model.n) <= 1) ? model.n : A2(
					$elm$core$List$filter,
					function (b) {
						return !_Utils_eq(b, bid);
					},
					model.n)) : _Utils_ap(
					model.n,
					_List_fromArray(
						[bid]));
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{n: newList}),
					$elm$core$Platform$Cmd$none);
			case 32:
				var _v12 = model.A;
				if (_v12.$ === 1) {
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{n: _List_Nil, m: false, J: _List_Nil}),
						$elm$core$Platform$Cmd$none);
				} else {
					var editedPieceId = _v12.a;
					var newBrickIds = model.n;
					var removedBrickIds = A2(
						$elm$core$Maybe$withDefault,
						_List_Nil,
						A2(
							$elm$core$Maybe$map,
							function (p) {
								return A2(
									$elm$core$List$filter,
									function (bid) {
										return !A2($elm$core$List$member, bid, newBrickIds);
									},
									p.u);
							},
							$elm$core$List$head(
								A2(
									$elm$core$List$filter,
									function (p) {
										return _Utils_eq(p.a, editedPieceId);
									},
									model.d))));
					var updatedExisting = A2(
						$elm$core$List$map,
						function (p) {
							return _Utils_eq(p.a, editedPieceId) ? _Utils_update(
								p,
								{u: newBrickIds}) : _Utils_update(
								p,
								{
									u: A2(
										$elm$core$List$filter,
										function (bid) {
											return !A2($elm$core$List$member, bid, newBrickIds);
										},
										p.u)
								});
						},
						model.d);
					var maxIdNum = A3(
						$elm$core$List$foldl,
						F2(
							function (p, acc) {
								var _v14 = $elm$core$String$toInt(
									A2($elm$core$String$dropLeft, 1, p.a));
								if (!_v14.$) {
									var n = _v14.a;
									return A2($elm$core$Basics$max, n, acc);
								} else {
									return acc;
								}
							}),
						0,
						model.d);
					var newSinglePieces = A2(
						$elm$core$List$indexedMap,
						F2(
							function (i, bid) {
								var newId = 'p' + $elm$core$String$fromInt((maxIdNum + i) + 1);
								var _v13 = A2($elm$core$Dict$get, bid, model.aH);
								if (!_v13.$) {
									var brick = _v13.a;
									return {
										u: _List_fromArray(
											[bid]),
										E: _List_fromArray(
											[
												A5($author$project$Main$BrickRef, bid, brick.h, brick.i, brick.l, brick.j)
											]),
										j: brick.j,
										a: newId,
										G: '/api/s/' + (model.D + ('/piece/' + (newId + '.png'))),
										au: '/api/s/' + (model.D + ('/piece_outline/' + (newId + '.png'))),
										z: _List_Nil,
										l: brick.l,
										h: brick.h,
										i: brick.i
									};
								} else {
									return {
										u: _List_fromArray(
											[bid]),
										E: _List_Nil,
										j: 0,
										a: newId,
										G: '/api/s/' + (model.D + ('/piece/' + (newId + '.png'))),
										au: '/api/s/' + (model.D + ('/piece_outline/' + (newId + '.png'))),
										z: _List_Nil,
										l: 0,
										h: 0,
										i: 0
									};
								}
							}),
						removedBrickIds);
					var allPieces = A2(
						$elm$core$List$map,
						A2($author$project$Main$recalcPieceBbox, model.D, model.aH),
						A2(
							$elm$core$List$filter,
							function (p) {
								return !$elm$core$List$isEmpty(p.u);
							},
							_Utils_ap(updatedExisting, newSinglePieces)));
					var validIds = A2(
						$elm$core$List$map,
						function ($) {
							return $.a;
						},
						allPieces);
					var updatedWaves = A2(
						$elm$core$List$map,
						function (w) {
							return _Utils_update(
								w,
								{
									b: A2(
										$elm$core$List$filter,
										function (pid) {
											return A2($elm$core$List$member, pid, validIds);
										},
										w.b)
								});
						},
						model.c);
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{
								n: _List_Nil,
								m: false,
								J: _List_Nil,
								y: 2,
								d: allPieces,
								x: true,
								A: $elm$core$Maybe$Just(editedPieceId),
								c: updatedWaves
							}),
						A2($author$project$Main$recomputePiecePolygons, model.D, allPieces));
				}
			case 33:
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{n: _List_Nil, m: false, J: _List_Nil}),
					$elm$core$Platform$Cmd$none);
			case 34:
				if (!msg.a.$) {
					var pairs = msg.a.a;
					var polyDict = $elm$core$Dict$fromList(pairs);
					var updatedPieces = A2(
						$elm$core$List$map,
						function (p) {
							var _v15 = A2($elm$core$Dict$get, p.a, polyDict);
							if (!_v15.$) {
								var poly = _v15.a;
								return _Utils_update(
									p,
									{z: poly});
							} else {
								return p;
							}
						},
						model.d);
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{s: model.s + 1, d: updatedPieces, x: false}),
						$elm$core$Platform$Cmd$none);
				} else {
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{x: false}),
						$elm$core$Platform$Cmd$none);
				}
			case 35:
				var s = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{aW: s}),
					$elm$core$Platform$Cmd$none);
			case 36:
				var s = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{aI: s}),
					$elm$core$Platform$Cmd$none);
			case 37:
				var s = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{af: s}),
					$elm$core$Platform$Cmd$none);
			case 38:
				var s = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{aJ: s}),
					$elm$core$Platform$Cmd$none);
			case 39:
				var s = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{aK: s}),
					$elm$core$Platform$Cmd$none);
			case 40:
				var wavesJson = A2(
					$elm$json$Json$Encode$list,
					function (_v17) {
						var idx = _v17.a;
						var wv = _v17.b;
						return $elm$json$Json$Encode$object(
							_List_fromArray(
								[
									_Utils_Tuple2(
									'wave',
									$elm$json$Json$Encode$int(idx + 1)),
									_Utils_Tuple2(
									'pieceIds',
									A2($elm$json$Json$Encode$list, $elm$json$Json$Encode$string, wv.b))
								]));
					},
					A2($elm$core$List$indexedMap, $elm$core$Tuple$pair, model.c));
				var outlinesJson = A2(
					$elm$json$Json$Encode$list,
					function (piece) {
						return $elm$json$Json$Encode$object(
							_List_fromArray(
								[
									_Utils_Tuple2(
									'points',
									A2(
										$elm$json$Json$Encode$list,
										function (_v16) {
											var x = _v16.a;
											var y = _v16.b;
											return A2(
												$elm$json$Json$Encode$list,
												$elm$json$Json$Encode$float,
												_List_fromArray(
													[x, y]));
										},
										piece.z))
								]));
					},
					model.d);
				var groupsJson = A2(
					$elm$json$Json$Encode$list,
					function (g) {
						return $elm$json$Json$Encode$object(
							_List_fromArray(
								[
									_Utils_Tuple2(
									'pieceIds',
									A2($elm$json$Json$Encode$list, $elm$json$Json$Encode$string, g.b))
								]));
					},
					model.f);
				var exportHeight = A2(
					$elm$core$Maybe$withDefault,
					900,
					$elm$core$String$toInt(model.aW));
				var payload = $elm$json$Json$Encode$object(
					_List_fromArray(
						[
							_Utils_Tuple2('waves', wavesJson),
							_Utils_Tuple2('outlines', outlinesJson),
							_Utils_Tuple2('groups', groupsJson),
							_Utils_Tuple2(
							'export_canvas_height',
							$elm$json$Json$Encode$int(exportHeight)),
							_Utils_Tuple2(
							'placement',
							$elm$json$Json$Encode$object(
								_List_fromArray(
									[
										_Utils_Tuple2(
										'location',
										$elm$json$Json$Encode$string(model.aI)),
										_Utils_Tuple2(
										'position',
										$elm$json$Json$Encode$int(
											A2(
												$elm$core$Maybe$withDefault,
												0,
												$elm$core$String$toInt(model.aJ)))),
										_Utils_Tuple2(
										'houseName',
										$elm$json$Json$Encode$string(model.af)),
										_Utils_Tuple2(
										'spacing',
										$elm$json$Json$Encode$float(
											A2(
												$elm$core$Maybe$withDefault,
												12.0,
												$elm$core$String$toFloat(model.aK))))
									])))
						]));
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{S: true}),
					$elm$http$Http$riskyRequest(
						{
							aG: $elm$http$Http$jsonBody(payload),
							as: $elm$http$Http$expectWhatever($author$project$Main$GotExportResponse),
							bi: _List_Nil,
							bl: 'POST',
							bF: $elm$core$Maybe$Just((10 * 60) * 1000),
							bG: $elm$core$Maybe$Nothing,
							aA: '/api/s/' + (model.D + '/export')
						}));
			case 41:
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{S: false}),
					$elm$core$Platform$Cmd$none);
			case 42:
				var brickId = msg.a;
				return _Utils_Tuple2(
					model,
					$author$project$Main$logBrick(
						$elm$json$Json$Encode$object(
							_List_fromArray(
								[
									_Utils_Tuple2(
									'brickId',
									$elm$json$Json$Encode$string(brickId)),
									_Utils_Tuple2(
									'pieceId',
									A2(
										$elm$core$Maybe$withDefault,
										$elm$json$Json$Encode$null,
										A2(
											$elm$core$Maybe$map,
											A2(
												$elm$core$Basics$composeR,
												function ($) {
													return $.a;
												},
												$elm$json$Json$Encode$string),
											$elm$core$List$head(
												A2(
													$elm$core$List$filter,
													function (p) {
														return A2(
															$elm$core$List$any,
															function (br) {
																return _Utils_eq(br.a, brickId);
															},
															p.E);
													},
													model.d)))))
								]))));
			case 43:
				var pid = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							F: $elm$core$Maybe$Just(pid)
						}),
					$elm$core$Platform$Cmd$none);
			case 44:
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{Q: $elm$core$Maybe$Nothing, R: $elm$core$Maybe$Nothing, F: $elm$core$Maybe$Nothing}),
					$elm$core$Platform$Cmd$none);
			case 45:
				var waveId = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							Q: $elm$core$Maybe$Nothing,
							R: $elm$core$Maybe$Just(waveId)
						}),
					$elm$core$Platform$Cmd$none);
			case 46:
				var pid = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							Q: $elm$core$Maybe$Just(pid)
						}),
					$elm$core$Platform$Cmd$none);
			case 47:
				var targetWaveId = msg.a;
				var _v18 = model.F;
				if (_v18.$ === 1) {
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{Q: $elm$core$Maybe$Nothing, R: $elm$core$Maybe$Nothing}),
						$elm$core$Platform$Cmd$none);
				} else {
					var pid = _v18.a;
					var targetIsLocked = function () {
						if (!targetWaveId.$) {
							var wid = targetWaveId.a;
							return A2(
								$elm$core$List$any,
								function (wv) {
									return _Utils_eq(wv.a, wid) && wv.g;
								},
								model.c);
						} else {
							return false;
						}
					}();
					var maybeGroup = $elm$core$List$head(
						A2(
							$elm$core$List$filter,
							function (g) {
								return A2($elm$core$List$member, pid, g.b);
							},
							model.f));
					var pidsToMove = function () {
						if (!maybeGroup.$) {
							var g = maybeGroup.a;
							return g.b;
						} else {
							return _List_fromArray(
								[pid]);
						}
					}();
					var sourceIsLocked = A2(
						$elm$core$List$any,
						function (wv) {
							return A2(
								$elm$core$List$any,
								function (p) {
									return A2($elm$core$List$member, p, pidsToMove);
								},
								wv.b) && wv.g;
						},
						model.c);
					var insertBefore = model.Q;
					var insertInto = function (wvPids) {
						var filtered = A2(
							$elm$core$List$filter,
							function (p) {
								return !A2($elm$core$List$member, p, pidsToMove);
							},
							wvPids);
						if (!insertBefore.$) {
							var beforeId = insertBefore.a;
							return A2($elm$core$List$member, beforeId, pidsToMove) ? _Utils_ap(filtered, pidsToMove) : A2(
								$elm$core$List$concatMap,
								function (p) {
									return _Utils_eq(p, beforeId) ? _Utils_ap(
										pidsToMove,
										_List_fromArray(
											[p])) : _List_fromArray(
										[p]);
								},
								filtered);
						} else {
							return _Utils_ap(filtered, pidsToMove);
						}
					};
					var newWaves = (targetIsLocked || sourceIsLocked) ? model.c : A2(
						$elm$core$List$map,
						function (wv) {
							if (!targetWaveId.$) {
								var wid = targetWaveId.a;
								return _Utils_eq(wv.a, wid) ? _Utils_update(
									wv,
									{
										b: insertInto(wv.b)
									}) : wv;
							} else {
								return wv;
							}
						},
						A2(
							$elm$core$List$map,
							function (wv) {
								return _Utils_update(
									wv,
									{
										b: A2(
											$elm$core$List$filter,
											function (p) {
												return !A2($elm$core$List$member, p, pidsToMove);
											},
											wv.b)
									});
							},
							model.c));
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{Q: $elm$core$Maybe$Nothing, R: $elm$core$Maybe$Nothing, F: $elm$core$Maybe$Nothing, c: newWaves}),
						$elm$core$Platform$Cmd$none);
				}
			case 48:
				var wid = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							c: A2(
								$elm$core$List$map,
								function (w) {
									return _Utils_eq(w.a, wid) ? _Utils_update(
										w,
										{g: !w.g}) : w;
								},
								model.c)
						}),
					$elm$core$Platform$Cmd$none);
			case 49:
				var gid = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							f: A2(
								$elm$core$List$map,
								function (g) {
									return _Utils_eq(g.a, gid) ? _Utils_update(
										g,
										{g: !g.g}) : g;
								},
								model.f)
						}),
					$elm$core$Platform$Cmd$none);
			case 50:
				var newGroup = {
					r: $author$project$Main$defaultHue(model.I - 1),
					a: model.I,
					g: false,
					N: 'Group ' + $elm$core$String$fromInt(model.I),
					b: _List_Nil
				};
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							f: _Utils_ap(
								model.f,
								_List_fromArray(
									[newGroup])),
							I: model.I + 1,
							C: $elm$core$Maybe$Just(newGroup.a)
						}),
					$elm$core$Platform$Cmd$none);
			case 51:
				var mgid = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{C: mgid}),
					$elm$core$Platform$Cmd$none);
			case 52:
				var gid = msg.a;
				var newGroups = A2(
					$elm$core$List$filter,
					function (g) {
						return !_Utils_eq(g.a, gid);
					},
					model.f);
				var newSelectedGroupId = _Utils_eq(
					model.C,
					$elm$core$Maybe$Just(gid)) ? A2(
					$elm$core$Maybe$map,
					function ($) {
						return $.a;
					},
					$elm$core$List$head(newGroups)) : model.C;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{f: newGroups, C: newSelectedGroupId}),
					$elm$core$Platform$Cmd$none);
			case 53:
				var gid = msg.a;
				var dir = msg.b;
				var moveItem = function (lst) {
					var indexed = A2($elm$core$List$indexedMap, $elm$core$Tuple$pair, lst);
					var idx = A2(
						$elm$core$Maybe$withDefault,
						0,
						A2(
							$elm$core$Maybe$map,
							$elm$core$Tuple$first,
							$elm$core$List$head(
								A2(
									$elm$core$List$filter,
									function (_v24) {
										var g = _v24.b;
										return _Utils_eq(g.a, gid);
									},
									indexed))));
					var item = $elm$core$List$head(
						A2($elm$core$List$drop, idx, lst));
					var newIdx = A2(
						$elm$core$Basics$max,
						0,
						A2(
							$elm$core$Basics$min,
							$elm$core$List$length(lst) - 1,
							idx + dir));
					var without = _Utils_ap(
						A2($elm$core$List$take, idx, lst),
						A2($elm$core$List$drop, idx + 1, lst));
					if (!item.$) {
						var g = item.a;
						return _Utils_ap(
							A2($elm$core$List$take, newIdx, without),
							_Utils_ap(
								_List_fromArray(
									[g]),
								A2($elm$core$List$drop, newIdx, without)));
					} else {
						return lst;
					}
				};
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							f: moveItem(model.f)
						}),
					$elm$core$Platform$Cmd$none);
			case 54:
				var pid = msg.a;
				var _v25 = model.C;
				if (_v25.$ === 1) {
					return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
				} else {
					var gid = _v25.a;
					var alreadyIn = A2(
						$elm$core$List$any,
						function (g) {
							return _Utils_eq(g.a, gid) && A2($elm$core$List$member, pid, g.b);
						},
						model.f);
					var updatedGroups = A2(
						$elm$core$List$map,
						function (g) {
							return _Utils_eq(g.a, gid) ? (alreadyIn ? _Utils_update(
								g,
								{
									b: A2(
										$elm$core$List$filter,
										function (p) {
											return !_Utils_eq(p, pid);
										},
										g.b)
								}) : _Utils_update(
								g,
								{
									b: _Utils_ap(
										g.b,
										_List_fromArray(
											[pid]))
								})) : ((!alreadyIn) ? _Utils_update(
								g,
								{
									b: A2(
										$elm$core$List$filter,
										function (p) {
											return !_Utils_eq(p, pid);
										},
										g.b)
								}) : g);
						},
						model.f);
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{f: updatedGroups}),
						$elm$core$Platform$Cmd$none);
				}
			case 55:
				var mgid = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							ae: $elm$core$Maybe$Just(mgid)
						}),
					$elm$core$Platform$Cmd$none);
			case 56:
				var mgid = msg.a;
				var _v26 = model.F;
				if (_v26.$ === 1) {
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{ae: $elm$core$Maybe$Nothing}),
						$elm$core$Platform$Cmd$none);
				} else {
					var pid = _v26.a;
					var updatedGroups = function () {
						if (mgid.$ === 1) {
							return A2(
								$elm$core$List$map,
								function (g) {
									return _Utils_update(
										g,
										{
											b: A2(
												$elm$core$List$filter,
												$elm$core$Basics$neq(pid),
												g.b)
										});
								},
								model.f);
						} else {
							var gid = mgid.a;
							return A2(
								$elm$core$List$map,
								function (g) {
									return _Utils_eq(g.a, gid) ? (A2($elm$core$List$member, pid, g.b) ? g : _Utils_update(
										g,
										{
											b: _Utils_ap(
												g.b,
												_List_fromArray(
													[pid]))
										})) : _Utils_update(
										g,
										{
											b: A2(
												$elm$core$List$filter,
												$elm$core$Basics$neq(pid),
												g.b)
										});
								},
								model.f);
						}
					}();
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{ae: $elm$core$Maybe$Nothing, F: $elm$core$Maybe$Nothing, f: updatedGroups}),
						$elm$core$Platform$Cmd$none);
				}
			case 57:
				var gid = msg.a;
				var wid = msg.b;
				var _v28 = $elm$core$List$head(
					A2(
						$elm$core$List$filter,
						function (g) {
							return _Utils_eq(g.a, gid);
						},
						model.f));
				if (_v28.$ === 1) {
					return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
				} else {
					var group = _v28.a;
					var targetLocked = A2(
						$elm$core$List$any,
						function (w) {
							return _Utils_eq(w.a, wid) && w.g;
						},
						model.c);
					var pids = group.b;
					var alreadyAll = (!$elm$core$List$isEmpty(pids)) && A2(
						$elm$core$List$all,
						function (pid) {
							return A2(
								$elm$core$List$any,
								function (w) {
									return _Utils_eq(w.a, wid) && A2($elm$core$List$member, pid, w.b);
								},
								model.c);
						},
						pids);
					var updatedWaves = targetLocked ? model.c : (alreadyAll ? A2(
						$elm$core$List$map,
						function (w) {
							return _Utils_eq(w.a, wid) ? _Utils_update(
								w,
								{
									b: A2(
										$elm$core$List$filter,
										function (p) {
											return !A2($elm$core$List$member, p, pids);
										},
										w.b)
								}) : w;
						},
						model.c) : A2(
						$elm$core$List$map,
						function (w) {
							return _Utils_eq(w.a, wid) ? _Utils_update(
								w,
								{
									b: _Utils_ap(w.b, pids)
								}) : w;
						},
						A2(
							$elm$core$List$map,
							function (w) {
								return _Utils_update(
									w,
									{
										b: A2(
											$elm$core$List$filter,
											function (p) {
												return !A2($elm$core$List$member, p, pids);
											},
											w.b)
									});
							},
							model.c)));
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{c: updatedWaves}),
						targetLocked ? $elm$core$Platform$Cmd$none : $author$project$Main$scrollTrayToEnd);
				}
			case 12:
				var viewport = msg.a;
				var waveTrayOffset = 48;
				var vh = viewport.bI.j;
				var waveTrayHeight = (vh - waveTrayOffset) * 0.12;
				var bottomPadding = 16;
				var availableH = (model.e === 5) ? ((vh - waveTrayHeight) - bottomPadding) : (vh - bottomPadding);
				var _v29 = model.o;
				if (_v29.$ === 2) {
					var response = _v29.a;
					var svgH = response.aq.j + 20;
					var scale = (availableH * model._) / (svgH * 15.5);
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{ap: availableH, aR: scale}),
						$author$project$Main$scrollToBottom);
				} else {
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{ap: availableH}),
						$elm$core$Platform$Cmd$none);
				}
			case 58:
				var x = msg.a;
				var y = msg.b;
				return (!_Utils_eq(model.k, $elm$core$Maybe$Nothing)) ? _Utils_Tuple2(
					_Utils_update(
						model,
						{
							W: $elm$core$Maybe$Just(
								{aC: x, an: x, aD: y, ao: y})
						}),
					$elm$core$Platform$Cmd$none) : _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
			case 59:
				var x = msg.a;
				var y = msg.b;
				var _v30 = model.W;
				if (_v30.$ === 1) {
					return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
				} else {
					var ls = _v30.a;
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{
								W: $elm$core$Maybe$Just(
									_Utils_update(
										ls,
										{an: x, ao: y}))
							}),
						$elm$core$Platform$Cmd$none);
				}
			case 60:
				var _v31 = model.W;
				if (_v31.$ === 1) {
					return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
				} else {
					var ls = _v31.a;
					var isDrag = ($elm$core$Basics$abs(ls.an - ls.aC) > 5) || ($elm$core$Basics$abs(ls.ao - ls.aD) > 5);
					var cleared = _Utils_update(
						model,
						{W: $elm$core$Maybe$Nothing});
					if (!isDrag) {
						return _Utils_Tuple2(cleared, $elm$core$Platform$Cmd$none);
					} else {
						var _v32 = model.k;
						if (_v32.$ === 1) {
							return _Utils_Tuple2(cleared, $elm$core$Platform$Cmd$none);
						} else {
							var wid = _v32.a;
							var ly1 = A2($elm$core$Basics$max, ls.aD, ls.ao);
							var ly0 = A2($elm$core$Basics$min, ls.aD, ls.ao);
							var lx1 = A2($elm$core$Basics$max, ls.aC, ls.an);
							var lx0 = A2($elm$core$Basics$min, ls.aC, ls.an);
							var selectedIds = A2(
								$elm$core$List$map,
								function ($) {
									return $.a;
								},
								A2(
									$elm$core$List$filter,
									function (p) {
										return (_Utils_cmp(p.h, lx1) < 0) && ((_Utils_cmp(p.h + p.l, lx0) > 0) && ((_Utils_cmp(p.i, ly1) < 0) && (_Utils_cmp(p.i + p.j, ly0) > 0)));
									},
									model.d));
							var updatedWaves = A3(
								$elm$core$List$foldl,
								F2(
									function (pid, waves) {
										var tgtLocked = A2(
											$elm$core$Maybe$withDefault,
											false,
											A2(
												$elm$core$Maybe$map,
												function ($) {
													return $.g;
												},
												$elm$core$List$head(
													A2(
														$elm$core$List$filter,
														function (w) {
															return _Utils_eq(w.a, wid);
														},
														waves))));
										var srcLocked = A2(
											$elm$core$List$any,
											function (w) {
												return w.g && A2($elm$core$List$member, pid, w.b);
											},
											waves);
										var alreadyIn = A2(
											$elm$core$Maybe$withDefault,
											false,
											A2(
												$elm$core$Maybe$map,
												function (w) {
													return A2($elm$core$List$member, pid, w.b);
												},
												$elm$core$List$head(
													A2(
														$elm$core$List$filter,
														function (w) {
															return _Utils_eq(w.a, wid);
														},
														waves))));
										return (tgtLocked || ((!alreadyIn) && srcLocked)) ? waves : (alreadyIn ? waves : A2(
											$elm$core$List$map,
											function (w) {
												return _Utils_eq(w.a, wid) ? _Utils_update(
													w,
													{
														b: _Utils_ap(
															w.b,
															_List_fromArray(
																[pid]))
													}) : _Utils_update(
													w,
													{
														b: A2(
															$elm$core$List$filter,
															function (p) {
																return !_Utils_eq(p, pid);
															},
															w.b)
													});
											},
											waves));
									}),
								model.c,
								selectedIds);
							return _Utils_Tuple2(
								_Utils_update(
									cleared,
									{c: updatedWaves}),
								$elm$core$Platform$Cmd$none);
						}
					}
				}
			case 61:
				var z = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{aE: z}),
					$elm$core$Platform$Cmd$none);
			case 62:
				var b = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{aZ: b}),
					$elm$core$Platform$Cmd$none);
			case 63:
				var s = msg.a;
				var _v33 = $elm$core$String$toFloat(s);
				if (!_v33.$) {
					var h = _v33.a;
					return (h > 0) ? _Utils_Tuple2(
						_Utils_update(
							model,
							{_: h}),
						A2($elm$core$Task$perform, $author$project$Main$GotViewport, $elm$browser$Browser$Dom$getViewport)) : _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
				} else {
					return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
				}
			case 64:
				var target = msg.a;
				var px = msg.b;
				var py = msg.c;
				var hueOnly = _Utils_eq(target, $author$project$Main$GridColorTarget) || _Utils_eq(target, $author$project$Main$OutlineColorTarget);
				var innerH = hueOnly ? 20 : 96;
				var _v34 = function () {
					switch (target.$) {
						case 0:
							var waveId = target.a;
							return A2(
								$elm$core$Maybe$withDefault,
								_Utils_Tuple2(0, 0.3),
								A2(
									$elm$core$Maybe$map,
									function (w) {
										return _Utils_Tuple2(w.r, w.ai);
									},
									$elm$core$List$head(
										A2(
											$elm$core$List$filter,
											function (w) {
												return _Utils_eq(w.a, waveId);
											},
											model.c))));
						case 1:
							var groupId = target.a;
							return A2(
								$elm$core$Maybe$withDefault,
								_Utils_Tuple2(0, 1.0),
								A2(
									$elm$core$Maybe$map,
									function (g) {
										return _Utils_Tuple2(g.r, 1.0);
									},
									$elm$core$List$head(
										A2(
											$elm$core$List$filter,
											function (g) {
												return _Utils_eq(g.a, groupId);
											},
											model.f))));
						case 2:
							return _Utils_Tuple2(model.ah, 1.0);
						default:
							return _Utils_Tuple2(model.aj, 1.0);
					}
				}();
				var currentHue = _v34.a;
				var currentOpacity = _v34.b;
				var panelX = (px - 10) - ((currentHue / 360) * 240);
				var panelY = (py - 10) - ((1 - currentOpacity) * innerH);
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							O: $elm$core$Maybe$Just(
								{a1: hueOnly, a5: panelX, a6: panelY, bE: target})
						}),
					$elm$core$Platform$Cmd$none);
			case 65:
				var mx = msg.a;
				var my = msg.b;
				var _v36 = model.O;
				if (_v36.$ === 1) {
					return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
				} else {
					var cp = _v36.a;
					var newOpacity = cp.a1 ? 1.0 : A3($elm$core$Basics$clamp, 0.05, 1.0, 1.0 - (((my - cp.a6) - 10) / 96));
					var localX = (mx - cp.a5) - 10;
					var newHue = (localX < 20) ? (-2) : ((localX < 40) ? (-1) : A3($elm$core$Basics$clamp, 0, 360, ((localX - 40) / 240) * 360));
					var _v37 = cp.bE;
					switch (_v37.$) {
						case 0:
							var waveId = _v37.a;
							return _Utils_Tuple2(
								_Utils_update(
									model,
									{
										c: A2(
											$elm$core$List$map,
											function (w) {
												return _Utils_eq(w.a, waveId) ? _Utils_update(
													w,
													{r: newHue, ai: newOpacity}) : w;
											},
											model.c)
									}),
								$elm$core$Platform$Cmd$none);
						case 1:
							var groupId = _v37.a;
							return _Utils_Tuple2(
								_Utils_update(
									model,
									{
										f: A2(
											$elm$core$List$map,
											function (g) {
												return _Utils_eq(g.a, groupId) ? _Utils_update(
													g,
													{r: newHue}) : g;
											},
											model.f)
									}),
								$elm$core$Platform$Cmd$none);
						case 2:
							return _Utils_Tuple2(
								_Utils_update(
									model,
									{ah: newHue}),
								$elm$core$Platform$Cmd$none);
						default:
							return _Utils_Tuple2(
								_Utils_update(
									model,
									{aj: newHue}),
								$elm$core$Platform$Cmd$none);
					}
				}
			case 18:
				var target = msg.a;
				var hue = msg.b;
				var updated = function () {
					switch (target.$) {
						case 2:
							return _Utils_update(
								model,
								{O: $elm$core$Maybe$Nothing, ah: hue});
						case 3:
							return _Utils_update(
								model,
								{O: $elm$core$Maybe$Nothing, aj: hue});
						case 0:
							var wid = target.a;
							return _Utils_update(
								model,
								{
									O: $elm$core$Maybe$Nothing,
									c: A2(
										$elm$core$List$map,
										function (w) {
											return _Utils_eq(w.a, wid) ? _Utils_update(
												w,
												{r: hue}) : w;
										},
										model.c)
								});
						default:
							var gid = target.a;
							return _Utils_update(
								model,
								{
									O: $elm$core$Maybe$Nothing,
									f: A2(
										$elm$core$List$map,
										function (g) {
											return _Utils_eq(g.a, gid) ? _Utils_update(
												g,
												{r: hue}) : g;
										},
										model.f)
								});
					}
				}();
				return _Utils_Tuple2(updated, $elm$core$Platform$Cmd$none);
			case 66:
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{O: $elm$core$Maybe$Nothing}),
					$elm$core$Platform$Cmd$none);
			case 67:
				var delta = msg.a;
				return _Utils_Tuple2(
					model,
					A2(
						$elm$core$Task$attempt,
						function (_v39) {
							return $author$project$Main$NoOp;
						},
						A2(
							$elm$core$Task$andThen,
							function (vp) {
								return A3($elm$browser$Browser$Dom$setViewportOf, 'wave-tray-scroll', vp.bI.h + delta, 0);
							},
							$elm$browser$Browser$Dom$getViewportOf('wave-tray-scroll'))));
			default:
				return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
		}
	});
var $elm$html$Html$Attributes$stringProperty = F2(
	function (key, string) {
		return A2(
			_VirtualDom_property,
			key,
			$elm$json$Json$Encode$string(string));
	});
var $elm$html$Html$Attributes$class = $elm$html$Html$Attributes$stringProperty('className');
var $elm$html$Html$div = _VirtualDom_node('div');
var $elm$virtual_dom$VirtualDom$text = _VirtualDom_text;
var $elm$html$Html$text = $elm$virtual_dom$VirtualDom$text;
var $author$project$Main$viewBodyOverlay = function (model) {
	var msg = _Utils_eq(model.o, $author$project$Main$Loading) ? $elm$core$Maybe$Just('Parsing PDF\u2026') : ((model.y === 1) ? $elm$core$Maybe$Just('Generating puzzle\u2026') : (model.x ? $elm$core$Maybe$Just('Updating pieces\u2026') : (model.S ? $elm$core$Maybe$Just('Exporting\u2026') : $elm$core$Maybe$Nothing)));
	if (msg.$ === 1) {
		return $elm$html$Html$text('');
	} else {
		var label = msg.a;
		return A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$class('body-overlay')
				]),
			_List_fromArray(
				[
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('overlay-spinner')
						]),
					_List_Nil),
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('overlay-label')
						]),
					_List_fromArray(
						[
							$elm$html$Html$text(label)
						]))
				]));
	}
};
var $elm$html$Html$Attributes$id = $elm$html$Html$Attributes$stringProperty('id');
var $author$project$Main$LassoEnd = {$: 60};
var $author$project$Main$LassoMove = F2(
	function (a, b) {
		return {$: 59, a: a, b: b};
	});
var $author$project$Main$LassoStart = F2(
	function (a, b) {
		return {$: 58, a: a, b: b};
	});
var $author$project$Main$ModeBlueprint = 3;
var $author$project$Main$ModeExport = 6;
var $elm$virtual_dom$VirtualDom$attribute = F2(
	function (key, value) {
		return A2(
			_VirtualDom_attribute,
			_VirtualDom_noOnOrFormAction(key),
			_VirtualDom_noJavaScriptOrHtmlUri(value));
	});
var $elm$html$Html$Attributes$attribute = $elm$virtual_dom$VirtualDom$attribute;
var $elm$svg$Svg$Attributes$class = _VirtualDom_attribute('class');
var $elm$svg$Svg$Attributes$fill = _VirtualDom_attribute('fill');
var $elm$core$String$fromFloat = _String_fromNumber;
var $elm$svg$Svg$trustedNode = _VirtualDom_nodeNS('http://www.w3.org/2000/svg');
var $elm$svg$Svg$g = $elm$svg$Svg$trustedNode('g');
var $elm$svg$Svg$Attributes$height = _VirtualDom_attribute('height');
var $elm$svg$Svg$image = $elm$svg$Svg$trustedNode('image');
var $elm$virtual_dom$VirtualDom$Normal = function (a) {
	return {$: 0, a: a};
};
var $elm$virtual_dom$VirtualDom$on = _VirtualDom_on;
var $elm$html$Html$Events$on = F2(
	function (event, decoder) {
		return A2(
			$elm$virtual_dom$VirtualDom$on,
			event,
			$elm$virtual_dom$VirtualDom$Normal(decoder));
	});
var $elm$svg$Svg$rect = $elm$svg$Svg$trustedNode('rect');
var $elm$svg$Svg$Attributes$stroke = _VirtualDom_attribute('stroke');
var $elm$svg$Svg$Attributes$strokeDasharray = _VirtualDom_attribute('stroke-dasharray');
var $elm$svg$Svg$Attributes$strokeWidth = _VirtualDom_attribute('stroke-width');
var $elm$svg$Svg$Attributes$style = _VirtualDom_attribute('style');
var $elm$svg$Svg$svg = $elm$svg$Svg$trustedNode('svg');
var $author$project$Main$GroupedPiece = F2(
	function (a, b) {
		return {$: 1, a: a, b: b};
	});
var $author$project$Main$SinglePiece = function (a) {
	return {$: 0, a: a};
};
var $author$project$Main$toPieceDisplays = F2(
	function (groups, pieceIds) {
		var go = F3(
			function (remaining, seen, acc) {
				go:
				while (true) {
					if (!remaining.b) {
						return $elm$core$List$reverse(acc);
					} else {
						var pid = remaining.a;
						var rest = remaining.b;
						var _v1 = $elm$core$List$head(
							A2(
								$elm$core$List$filter,
								function (g) {
									return (!$elm$core$List$isEmpty(g.b)) && A2($elm$core$List$member, pid, g.b);
								},
								groups));
						if (!_v1.$) {
							var g = _v1.a;
							if (A2($elm$core$List$member, g.a, seen)) {
								var $temp$remaining = rest,
									$temp$seen = seen,
									$temp$acc = acc;
								remaining = $temp$remaining;
								seen = $temp$seen;
								acc = $temp$acc;
								continue go;
							} else {
								var $temp$remaining = rest,
									$temp$seen = A2($elm$core$List$cons, g.a, seen),
									$temp$acc = A2(
									$elm$core$List$cons,
									A2(
										$author$project$Main$GroupedPiece,
										A2(
											$elm$core$Maybe$withDefault,
											pid,
											$elm$core$List$head(g.b)),
										g.b),
									acc);
								remaining = $temp$remaining;
								seen = $temp$seen;
								acc = $temp$acc;
								continue go;
							}
						} else {
							var $temp$remaining = rest,
								$temp$seen = seen,
								$temp$acc = A2(
								$elm$core$List$cons,
								$author$project$Main$SinglePiece(pid),
								acc);
							remaining = $temp$remaining;
							seen = $temp$seen;
							acc = $temp$acc;
							continue go;
						}
					}
				}
			});
		return A3(go, pieceIds, _List_Nil, _List_Nil);
	});
var $elm$svg$Svg$Attributes$viewBox = _VirtualDom_attribute('viewBox');
var $author$project$Main$ToggleBrickInEdit = function (a) {
	return {$: 31, a: a};
};
var $elm$svg$Svg$Attributes$fontSize = _VirtualDom_attribute('font-size');
var $elm$svg$Svg$Attributes$fontWeight = _VirtualDom_attribute('font-weight');
var $elm$html$Html$Events$onClick = function (msg) {
	return A2(
		$elm$html$Html$Events$on,
		'click',
		$elm$json$Json$Decode$succeed(msg));
};
var $elm$svg$Svg$Attributes$opacity = _VirtualDom_attribute('opacity');
var $elm$svg$Svg$Attributes$points = _VirtualDom_attribute('points');
var $elm$svg$Svg$polygon = $elm$svg$Svg$trustedNode('polygon');
var $elm$svg$Svg$text = $elm$virtual_dom$VirtualDom$text;
var $elm$svg$Svg$text_ = $elm$svg$Svg$trustedNode('text');
var $elm$svg$Svg$Attributes$width = _VirtualDom_attribute('width');
var $elm$svg$Svg$Attributes$x = _VirtualDom_attribute('x');
var $elm$svg$Svg$Attributes$y = _VirtualDom_attribute('y');
var $author$project$Main$viewBrickEditOverlay = F2(
	function (editBrickIds, brick) {
		var inEdit = A2($elm$core$List$member, brick.a, editBrickIds);
		var cls = inEdit ? 'brick-edit-in' : 'brick-edit-out';
		var absPoints = A2(
			$elm$core$List$map,
			function (_v1) {
				var x = _v1.a;
				var y = _v1.b;
				return _Utils_Tuple2(x + brick.h, y + brick.i);
			},
			brick.z);
		var pointsAttr = A2(
			$elm$core$String$join,
			' ',
			A2(
				$elm$core$List$map,
				function (_v0) {
					var x = _v0.a;
					var y = _v0.b;
					return $elm$core$String$fromFloat(x) + (',' + $elm$core$String$fromFloat(y));
				},
				absPoints));
		return $elm$core$List$isEmpty(absPoints) ? A2(
			$elm$svg$Svg$g,
			_List_Nil,
			_List_fromArray(
				[
					A2(
					$elm$svg$Svg$rect,
					_List_fromArray(
						[
							$elm$svg$Svg$Attributes$x(
							$elm$core$String$fromFloat(brick.h)),
							$elm$svg$Svg$Attributes$y(
							$elm$core$String$fromFloat(brick.i)),
							$elm$svg$Svg$Attributes$width('20'),
							$elm$svg$Svg$Attributes$height('20'),
							$elm$svg$Svg$Attributes$fill('red'),
							$elm$svg$Svg$Attributes$opacity('0.8')
						]),
					_List_Nil),
					A2(
					$elm$svg$Svg$text_,
					_List_fromArray(
						[
							$elm$svg$Svg$Attributes$x(
							$elm$core$String$fromFloat(brick.h + 2)),
							$elm$svg$Svg$Attributes$y(
							$elm$core$String$fromFloat(brick.i + 14)),
							$elm$svg$Svg$Attributes$fontSize('12'),
							$elm$svg$Svg$Attributes$fill('white'),
							$elm$svg$Svg$Attributes$fontWeight('bold')
						]),
					_List_fromArray(
						[
							$elm$svg$Svg$text('!' + brick.a)
						]))
				])) : A2(
			$elm$svg$Svg$polygon,
			_List_fromArray(
				[
					$elm$svg$Svg$Attributes$points(pointsAttr),
					$elm$svg$Svg$Attributes$class(cls),
					A2($elm$html$Html$Attributes$attribute, 'vector-effect', 'non-scaling-stroke'),
					$elm$html$Html$Events$onClick(
					$author$project$Main$ToggleBrickInEdit(brick.a))
				]),
			_List_Nil);
	});
var $author$project$Main$LogBrickClick = function (a) {
	return {$: 42, a: a};
};
var $author$project$Main$viewBrickOverlay = function (brick) {
	var absPoints = A2(
		$elm$core$List$map,
		function (_v1) {
			var x = _v1.a;
			var y = _v1.b;
			return _Utils_Tuple2(x + brick.h, y + brick.i);
		},
		brick.z);
	var pointsAttr = A2(
		$elm$core$String$join,
		' ',
		A2(
			$elm$core$List$map,
			function (_v0) {
				var x = _v0.a;
				var y = _v0.b;
				return $elm$core$String$fromFloat(x) + (',' + $elm$core$String$fromFloat(y));
			},
			absPoints));
	return $elm$core$List$isEmpty(absPoints) ? A2(
		$elm$svg$Svg$g,
		_List_Nil,
		_List_fromArray(
			[
				A2(
				$elm$svg$Svg$rect,
				_List_fromArray(
					[
						$elm$svg$Svg$Attributes$x(
						$elm$core$String$fromFloat(brick.h)),
						$elm$svg$Svg$Attributes$y(
						$elm$core$String$fromFloat(brick.i)),
						$elm$svg$Svg$Attributes$width('20'),
						$elm$svg$Svg$Attributes$height('20'),
						$elm$svg$Svg$Attributes$fill('red'),
						$elm$svg$Svg$Attributes$opacity('0.8')
					]),
				_List_Nil),
				A2(
				$elm$svg$Svg$text_,
				_List_fromArray(
					[
						$elm$svg$Svg$Attributes$x(
						$elm$core$String$fromFloat(brick.h + 2)),
						$elm$svg$Svg$Attributes$y(
						$elm$core$String$fromFloat(brick.i + 14)),
						$elm$svg$Svg$Attributes$fontSize('12'),
						$elm$svg$Svg$Attributes$fill('white'),
						$elm$svg$Svg$Attributes$fontWeight('bold')
					]),
				_List_fromArray(
					[
						$elm$svg$Svg$text('!' + brick.a)
					]))
			])) : A2(
		$elm$svg$Svg$polygon,
		_List_fromArray(
			[
				$elm$svg$Svg$Attributes$points(pointsAttr),
				$elm$svg$Svg$Attributes$fill('transparent'),
				A2($elm$html$Html$Attributes$attribute, 'vector-effect', 'non-scaling-stroke'),
				$elm$svg$Svg$Attributes$class('brick-overlay'),
				$elm$html$Html$Events$onClick(
				$author$project$Main$LogBrickClick(brick.a))
			]),
		_List_Nil);
};
var $elm$svg$Svg$line = $elm$svg$Svg$trustedNode('line');
var $elm$svg$Svg$Attributes$x1 = _VirtualDom_attribute('x1');
var $elm$svg$Svg$Attributes$x2 = _VirtualDom_attribute('x2');
var $elm$svg$Svg$Attributes$y1 = _VirtualDom_attribute('y1');
var $elm$svg$Svg$Attributes$y2 = _VirtualDom_attribute('y2');
var $author$project$Main$viewGrid = F4(
	function (cw, ch, color, houseUnitsHigh) {
		var gridStep = ch / houseUnitsHigh;
		var numH = $elm$core$Basics$floor(ch / gridStep) + 1;
		var hLines = A2(
			$elm$core$List$map,
			function (i) {
				var y = ch - (i * gridStep);
				return A2(
					$elm$svg$Svg$line,
					_List_fromArray(
						[
							$elm$svg$Svg$Attributes$x1(
							$elm$core$String$fromFloat(-gridStep)),
							$elm$svg$Svg$Attributes$y1(
							$elm$core$String$fromFloat(y)),
							$elm$svg$Svg$Attributes$x2(
							$elm$core$String$fromFloat(cw + gridStep)),
							$elm$svg$Svg$Attributes$y2(
							$elm$core$String$fromFloat(y)),
							$elm$svg$Svg$Attributes$stroke(color),
							$elm$svg$Svg$Attributes$strokeWidth('1'),
							A2($elm$html$Html$Attributes$attribute, 'vector-effect', 'non-scaling-stroke')
						]),
					_List_Nil);
			},
			A2($elm$core$List$range, -1, numH));
		var numV = $elm$core$Basics$floor(cw / gridStep) + 1;
		var vLines = A2(
			$elm$core$List$map,
			function (i) {
				var x = i * gridStep;
				return A2(
					$elm$svg$Svg$line,
					_List_fromArray(
						[
							$elm$svg$Svg$Attributes$x1(
							$elm$core$String$fromFloat(x)),
							$elm$svg$Svg$Attributes$y1(
							$elm$core$String$fromFloat(-gridStep)),
							$elm$svg$Svg$Attributes$x2(
							$elm$core$String$fromFloat(x)),
							$elm$svg$Svg$Attributes$y2(
							$elm$core$String$fromFloat(ch + gridStep)),
							$elm$svg$Svg$Attributes$stroke(color),
							$elm$svg$Svg$Attributes$strokeWidth('1'),
							A2($elm$html$Html$Attributes$attribute, 'vector-effect', 'non-scaling-stroke')
						]),
					_List_Nil);
			},
			A2($elm$core$List$range, -1, numV));
		return _Utils_ap(vLines, hLines);
	});
var $elm$svg$Svg$Attributes$strokeLinejoin = _VirtualDom_attribute('stroke-linejoin');
var $author$project$Main$viewPieceBlueprintPath = function (piece) {
	if ($elm$core$List$isEmpty(piece.z)) {
		return A2($elm$svg$Svg$g, _List_Nil, _List_Nil);
	} else {
		var pointsAttr = A2(
			$elm$core$String$join,
			' ',
			A2(
				$elm$core$List$map,
				function (_v0) {
					var x = _v0.a;
					var y = _v0.b;
					return $elm$core$String$fromFloat(x) + (',' + $elm$core$String$fromFloat(y));
				},
				piece.z));
		return A2(
			$elm$svg$Svg$polygon,
			_List_fromArray(
				[
					$elm$svg$Svg$Attributes$points(pointsAttr),
					$elm$svg$Svg$Attributes$fill('none'),
					$elm$svg$Svg$Attributes$stroke('white'),
					$elm$svg$Svg$Attributes$strokeWidth('4'),
					$elm$svg$Svg$Attributes$strokeLinejoin('round'),
					A2($elm$html$Html$Attributes$attribute, 'stroke-linecap', 'round'),
					A2($elm$html$Html$Attributes$attribute, 'vector-effect', 'non-scaling-stroke'),
					$elm$svg$Svg$Attributes$class('brick-path')
				]),
			_List_Nil);
	}
};
var $author$project$Main$viewPieceImage = F2(
	function (generation, piece) {
		return A2(
			$elm$svg$Svg$image,
			_List_fromArray(
				[
					$elm$svg$Svg$Attributes$x(
					$elm$core$String$fromFloat(piece.h)),
					$elm$svg$Svg$Attributes$y(
					$elm$core$String$fromFloat(piece.i)),
					$elm$svg$Svg$Attributes$width(
					$elm$core$String$fromFloat(piece.l)),
					$elm$svg$Svg$Attributes$height(
					$elm$core$String$fromFloat(piece.j)),
					A2(
					$elm$html$Html$Attributes$attribute,
					'href',
					piece.G + ('?v=' + $elm$core$String$fromInt(generation)))
				]),
			_List_Nil);
	});
var $elm$svg$Svg$Attributes$dominantBaseline = _VirtualDom_attribute('dominant-baseline');
var $elm$core$List$sortBy = _List_sortBy;
var $elm$svg$Svg$Attributes$textAnchor = _VirtualDom_attribute('text-anchor');
var $author$project$Main$viewPieceNumberLabel = F2(
	function (piece, pos) {
		var minDim = A2($elm$core$Basics$min, piece.l, piece.j);
		var label = $elm$core$String$fromInt(pos);
		var brickScore = function (b) {
			var bcy = b.i + (b.j / 2);
			var db = (piece.i + piece.j) - bcy;
			var dt = bcy - piece.i;
			var bcx = b.h + (b.l / 2);
			var dl = bcx - piece.h;
			var dr = (piece.h + piece.l) - bcx;
			return A2(
				$elm$core$Basics$min,
				A2($elm$core$Basics$min, dl, dr),
				A2($elm$core$Basics$min, dt, db));
		};
		var bestBrick = $elm$core$List$head(
			A2(
				$elm$core$List$sortBy,
				function (b) {
					return -brickScore(b);
				},
				piece.E));
		var _v0 = function () {
			if (!bestBrick.$) {
				var b = bestBrick.a;
				return _Utils_Tuple2(b.h + (b.l / 2), b.i + (b.j / 2));
			} else {
				return _Utils_Tuple2(piece.h + (piece.l / 2), piece.i + (piece.j / 2));
			}
		}();
		var rawCx = _v0.a;
		var rawCy = _v0.b;
		var _v2 = (minDim < 20) ? _Utils_Tuple2(14, '14') : ((minDim < 35) ? _Utils_Tuple2(18, '18') : _Utils_Tuple2(25, '25'));
		var fontSizeNum = _v2.a;
		var fontSizeStr = _v2.b;
		var halfFont = (fontSizeNum / 2) + 2;
		var cx = A2(
			$elm$core$Basics$max,
			piece.h + halfFont,
			A2($elm$core$Basics$min, (piece.h + piece.l) - halfFont, rawCx));
		var cy = A2(
			$elm$core$Basics$max,
			piece.i + halfFont,
			A2($elm$core$Basics$min, (piece.i + piece.j) - halfFont, rawCy));
		return A2(
			$elm$svg$Svg$g,
			_List_fromArray(
				[
					$elm$svg$Svg$Attributes$class('piece-number-label'),
					A2($elm$html$Html$Attributes$attribute, 'pointer-events', 'none')
				]),
			_List_fromArray(
				[
					A2(
					$elm$svg$Svg$text_,
					_List_fromArray(
						[
							$elm$svg$Svg$Attributes$x(
							$elm$core$String$fromFloat(cx)),
							$elm$svg$Svg$Attributes$y(
							$elm$core$String$fromFloat(cy)),
							$elm$svg$Svg$Attributes$textAnchor('middle'),
							$elm$svg$Svg$Attributes$dominantBaseline('central'),
							$elm$svg$Svg$Attributes$class('piece-num-shadow'),
							$elm$svg$Svg$Attributes$fontSize(fontSizeStr)
						]),
					_List_fromArray(
						[
							$elm$svg$Svg$text(label)
						])),
					A2(
					$elm$svg$Svg$text_,
					_List_fromArray(
						[
							$elm$svg$Svg$Attributes$x(
							$elm$core$String$fromFloat(cx)),
							$elm$svg$Svg$Attributes$y(
							$elm$core$String$fromFloat(cy)),
							$elm$svg$Svg$Attributes$textAnchor('middle'),
							$elm$svg$Svg$Attributes$dominantBaseline('central'),
							$elm$svg$Svg$Attributes$class('piece-num-text'),
							$elm$svg$Svg$Attributes$fontSize(fontSizeStr)
						]),
					_List_fromArray(
						[
							$elm$svg$Svg$text(label)
						]))
				]));
	});
var $author$project$Main$viewPieceOutline = F2(
	function (color, piece) {
		if ($elm$core$List$isEmpty(piece.z)) {
			return A2($elm$svg$Svg$g, _List_Nil, _List_Nil);
		} else {
			var pointsAttr = A2(
				$elm$core$String$join,
				' ',
				A2(
					$elm$core$List$map,
					function (_v0) {
						var x = _v0.a;
						var y = _v0.b;
						return $elm$core$String$fromFloat(x) + (',' + $elm$core$String$fromFloat(y));
					},
					piece.z));
			return A2(
				$elm$svg$Svg$polygon,
				_List_fromArray(
					[
						$elm$svg$Svg$Attributes$points(pointsAttr),
						$elm$svg$Svg$Attributes$fill('transparent'),
						$elm$svg$Svg$Attributes$stroke(color),
						$elm$svg$Svg$Attributes$strokeWidth('3'),
						$elm$svg$Svg$Attributes$strokeLinejoin('round'),
						A2($elm$html$Html$Attributes$attribute, 'vector-effect', 'non-scaling-stroke'),
						$elm$svg$Svg$Attributes$class('piece-outline'),
						A2($elm$html$Html$Attributes$attribute, 'pointer-events', 'none')
					]),
				_List_Nil);
		}
	});
var $author$project$Main$AssignGroupToWave = F2(
	function (a, b) {
		return {$: 57, a: a, b: b};
	});
var $author$project$Main$AssignPieceToGroup = function (a) {
	return {$: 54, a: a};
};
var $author$project$Main$AssignPieceToWave = function (a) {
	return {$: 26, a: a};
};
var $author$project$Main$SelectPiece = function (a) {
	return {$: 24, a: a};
};
var $author$project$Main$SetHoveredPiece = function (a) {
	return {$: 23, a: a};
};
var $elm$html$Html$Events$onMouseEnter = function (msg) {
	return A2(
		$elm$html$Html$Events$on,
		'mouseenter',
		$elm$json$Json$Decode$succeed(msg));
};
var $elm$html$Html$Events$onMouseLeave = function (msg) {
	return A2(
		$elm$html$Html$Events$on,
		'mouseleave',
		$elm$json$Json$Decode$succeed(msg));
};
var $author$project$Main$hslToRgb = function (hue) {
	var h = hue / 60;
	var i = $elm$core$Basics$floor(h);
	var f = h - i;
	var p = $elm$core$Basics$round(255 * f);
	var q = $elm$core$Basics$round(255 * (1 - f));
	var _v0 = A2($elm$core$Basics$modBy, 6, i);
	switch (_v0) {
		case 0:
			return _Utils_Tuple3(255, p, 0);
		case 1:
			return _Utils_Tuple3(q, 255, 0);
		case 2:
			return _Utils_Tuple3(0, 255, p);
		case 3:
			return _Utils_Tuple3(0, q, 255);
		case 4:
			return _Utils_Tuple3(p, 0, 255);
		default:
			return _Utils_Tuple3(255, 0, q);
	}
};
var $author$project$Main$waveColor = F2(
	function (hue, opacity) {
		if (_Utils_cmp(hue, -1.5) < 0) {
			return 'rgba(0,0,0,' + ($elm$core$String$fromFloat(opacity) + ')');
		} else {
			if (_Utils_cmp(hue, -0.5) < 0) {
				return 'rgba(255,255,255,' + ($elm$core$String$fromFloat(opacity) + ')');
			} else {
				var _v0 = $author$project$Main$hslToRgb(hue);
				var r = _v0.a;
				var g = _v0.b;
				var b = _v0.c;
				return 'rgba(' + ($elm$core$String$fromInt(r) + (',' + ($elm$core$String$fromInt(g) + (',' + ($elm$core$String$fromInt(b) + (',' + ($elm$core$String$fromFloat(opacity) + ')')))))));
			}
		}
	});
var $author$project$Main$viewPieceOverlay = function (appMode) {
	return function (hoveredId) {
		return function (selectedId) {
			return function (selectedWaveId) {
				return function (waves) {
					return function (groups) {
						return function (selectedGroupId) {
							return function (isLassoing) {
								return function (showOverlayFill) {
									return function (piece) {
										var maybeWave = $elm$core$List$head(
											A2(
												$elm$core$List$filter,
												function (w) {
													return w.V && A2($elm$core$List$member, piece.a, w.b);
												},
												waves));
										var maybeGroup = $elm$core$List$head(
											A2(
												$elm$core$List$filter,
												function (g) {
													return A2($elm$core$List$member, piece.a, g.b);
												},
												groups));
										var isHov = _Utils_eq(
											hoveredId,
											$elm$core$Maybe$Just(piece.a));
										var inWaveAssign = (appMode === 5) && (!_Utils_eq(selectedWaveId, $elm$core$Maybe$Nothing));
										var inGroupAssign = (appMode === 4) && (!_Utils_eq(selectedGroupId, $elm$core$Maybe$Nothing));
										var isSel = (!inWaveAssign) && ((!inGroupAssign) && _Utils_eq(
											selectedId,
											$elm$core$Maybe$Just(piece.a)));
										var fillStyle = function () {
											if (appMode === 4) {
												if (!maybeGroup.$) {
													var g = maybeGroup.a;
													if (showOverlayFill) {
														var eff = isHov ? A2($elm$core$Basics$min, 1.0, 0.35 + 0.15) : 0.35;
														return 'fill: ' + (A2($author$project$Main$waveColor, g.r, eff) + ';');
													} else {
														if (isHov) {
															return 'fill: rgba(64,120,255,0.2);';
														} else {
															return 'fill: transparent;';
														}
													}
												} else {
													return isHov ? 'fill: rgba(64,120,255,0.2);' : 'fill: transparent;';
												}
											} else {
												if ((appMode === 5) || (appMode === 6)) {
													if (!maybeWave.$) {
														var wv = maybeWave.a;
														if (showOverlayFill) {
															var eff = isHov ? A2($elm$core$Basics$min, 1.0, wv.ai + 0.3) : wv.ai;
															return 'fill: ' + (A2($author$project$Main$waveColor, wv.r, eff) + ';');
														} else {
															if (isHov) {
																return 'fill: rgba(64,120,255,0.2);';
															} else {
																return 'fill: transparent;';
															}
														}
													} else {
														return isHov ? 'fill: rgba(64,120,255,0.2);' : (isSel ? 'fill: rgba(64,120,255,0.45);' : 'fill: transparent;');
													}
												} else {
													if (isHov) {
														return 'fill: rgba(64,120,255,0.2);';
													} else {
														if (isSel) {
															return 'fill: rgba(64,120,255,0.45);';
														} else {
															return 'fill: transparent;';
														}
													}
												}
											}
										}();
										var clsStr = A2(
											$elm$core$String$join,
											' ',
											A2(
												$elm$core$List$filter,
												$elm$core$Basics$neq(''),
												_List_fromArray(
													[
														'piece-overlay',
														(isSel && _Utils_eq(maybeWave, $elm$core$Maybe$Nothing)) ? 'selected' : ''
													])));
										var clickMsg = function () {
											if (inGroupAssign) {
												return $author$project$Main$AssignPieceToGroup(piece.a);
											} else {
												if (inWaveAssign) {
													var _v1 = _Utils_Tuple2(maybeGroup, selectedWaveId);
													if ((!_v1.a.$) && (!_v1.b.$)) {
														var g = _v1.a.a;
														var wid = _v1.b.a;
														return A2($author$project$Main$AssignGroupToWave, g.a, wid);
													} else {
														return $author$project$Main$AssignPieceToWave(piece.a);
													}
												} else {
													return $author$project$Main$SelectPiece(piece.a);
												}
											}
										}();
										if ($elm$core$List$isEmpty(piece.z)) {
											return A2($elm$svg$Svg$g, _List_Nil, _List_Nil);
										} else {
											var pointsAttr = A2(
												$elm$core$String$join,
												' ',
												A2(
													$elm$core$List$map,
													function (_v0) {
														var x = _v0.a;
														var y = _v0.b;
														return $elm$core$String$fromFloat(x) + (',' + $elm$core$String$fromFloat(y));
													},
													piece.z));
											var pointerStyle = isLassoing ? 'pointer-events: none; ' : '';
											return A2(
												$elm$svg$Svg$polygon,
												_Utils_ap(
													_List_fromArray(
														[
															$elm$svg$Svg$Attributes$points(pointsAttr),
															$elm$svg$Svg$Attributes$class(clsStr),
															$elm$svg$Svg$Attributes$style(
															_Utils_ap(pointerStyle, fillStyle))
														]),
													isLassoing ? _List_Nil : _List_fromArray(
														[
															$elm$html$Html$Events$onClick(clickMsg),
															$elm$html$Html$Events$onMouseEnter(
															$author$project$Main$SetHoveredPiece(
																$elm$core$Maybe$Just(piece.a))),
															$elm$html$Html$Events$onMouseLeave(
															$author$project$Main$SetHoveredPiece($elm$core$Maybe$Nothing))
														])),
												_List_Nil);
										}
									};
								};
							};
						};
					};
				};
			};
		};
	};
};
var $author$project$Main$viewMainSvg = F2(
	function (response, model) {
		var showOverlayFill = ((model.e === 4) && model.aO) || (((model.e === 5) && model.ax) || ((model.e === 6) && model.ax));
		var piecePositions = $elm$core$Dict$fromList(
			A2(
				$elm$core$List$concatMap,
				function (wv) {
					return $elm$core$List$concat(
						A2(
							$elm$core$List$indexedMap,
							F2(
								function (i, display) {
									if (!display.$) {
										var pid = display.a;
										return _List_fromArray(
											[
												_Utils_Tuple2(pid, i + 1)
											]);
									} else {
										var allIds = display.b;
										return A2(
											$elm$core$List$map,
											function (pid) {
												return _Utils_Tuple2(pid, i + 1);
											},
											allIds);
									}
								}),
							A2($author$project$Main$toPieceDisplays, model.f, wv.b)));
				},
				model.c));
		var lassoRect = function () {
			var _v3 = model.W;
			if (_v3.$ === 1) {
				return _List_Nil;
			} else {
				var ls = _v3.a;
				var ry = A2($elm$core$Basics$min, ls.aD, ls.ao);
				var rx = A2($elm$core$Basics$min, ls.aC, ls.an);
				var rw = $elm$core$Basics$abs(ls.an - ls.aC);
				var rh = $elm$core$Basics$abs(ls.ao - ls.aD);
				return _List_fromArray(
					[
						A2(
						$elm$svg$Svg$rect,
						_List_fromArray(
							[
								$elm$svg$Svg$Attributes$x(
								$elm$core$String$fromFloat(rx)),
								$elm$svg$Svg$Attributes$y(
								$elm$core$String$fromFloat(ry)),
								$elm$svg$Svg$Attributes$width(
								$elm$core$String$fromFloat(rw)),
								$elm$svg$Svg$Attributes$height(
								$elm$core$String$fromFloat(rh)),
								$elm$svg$Svg$Attributes$fill('rgba(64,120,255,0.1)'),
								$elm$svg$Svg$Attributes$stroke('rgba(64,120,255,0.8)'),
								$elm$svg$Svg$Attributes$strokeWidth('1.5'),
								$elm$svg$Svg$Attributes$strokeDasharray('4 3'),
								A2($elm$html$Html$Attributes$attribute, 'vector-effect', 'non-scaling-stroke'),
								$elm$svg$Svg$Attributes$style('pointer-events: none;')
							]),
						_List_Nil)
					]);
			}
		}();
		var isLassoing = !_Utils_eq(model.W, $elm$core$Maybe$Nothing);
		var isGenerated = model.y === 2;
		var showComposite = response.a0 && ((!isGenerated) || (model.e === 1));
		var showPieceImages = ((model.e === 2) || ((model.e === 4) || ((model.e === 5) || (model.e === 6)))) && (isGenerated && (!$elm$core$List$isEmpty(model.d)));
		var hiddenPieceIds = (model.e === 5) ? A2(
			$elm$core$List$concatMap,
			function ($) {
				return $.b;
			},
			A2(
				$elm$core$List$filter,
				function (wv) {
					return !wv.V;
				},
				model.c)) : _List_Nil;
		var visiblePieces = function () {
			var filtered = A2(
				$elm$core$List$filter,
				function (p) {
					return !A2($elm$core$List$member, p.a, hiddenPieceIds);
				},
				model.d);
			var _v2 = model.F;
			if (!_v2.$) {
				var dragId = _v2.a;
				return _Utils_ap(
					A2(
						$elm$core$List$filter,
						function (p) {
							return !_Utils_eq(p.a, dragId);
						},
						filtered),
					A2(
						$elm$core$List$filter,
						function (p) {
							return _Utils_eq(p.a, dragId);
						},
						filtered));
			} else {
				return filtered;
			}
		}();
		var numberLabels = ((!model.m) && (isGenerated && (model.aw && ((model.e === 2) || ((model.e === 5) || (model.e === 6)))))) ? A2(
			$elm$core$List$filterMap,
			function (piece) {
				return A2(
					$elm$core$Maybe$map,
					$author$project$Main$viewPieceNumberLabel(piece),
					A2($elm$core$Dict$get, piece.a, piecePositions));
			},
			visiblePieces) : _List_Nil;
		var outlineLayer = ((!model.m) && (isGenerated && (model.aQ && ((model.e === 2) || ((model.e === 4) || ((model.e === 5) || (model.e === 6))))))) ? A2(
			$elm$core$List$map,
			$author$project$Main$viewPieceOutline(
				A2($author$project$Main$waveColor, model.aj, 1.0)),
			visiblePieces) : _List_Nil;
		var effectiveScale = model.aR * model.aE;
		var effectiveHoverId = (!_Utils_eq(model.F, $elm$core$Maybe$Nothing)) ? model.F : model.M;
		var pieceOverlays = ((!model.m) && isGenerated) ? A2(
			$elm$core$List$map,
			A9($author$project$Main$viewPieceOverlay, model.e, effectiveHoverId, model.A, model.k, model.c, model.f, model.C, isLassoing, showOverlayFill),
			visiblePieces) : _List_Nil;
		var editOverlays = model.m ? A2(
			$elm$core$List$map,
			$author$project$Main$viewBrickEditOverlay(model.n),
			response.E) : _List_Nil;
		var decodeLassoCoords = function (toMsg) {
			return A3(
				$elm$json$Json$Decode$map2,
				toMsg,
				A2(
					$elm$json$Json$Decode$map,
					function (x) {
						return (x / effectiveScale) - 200;
					},
					A2($elm$json$Json$Decode$field, 'offsetX', $elm$json$Json$Decode$float)),
				A2(
					$elm$json$Json$Decode$map,
					function (y) {
						return (y / effectiveScale) - 10;
					},
					A2($elm$json$Json$Decode$field, 'offsetY', $elm$json$Json$Decode$float)));
		};
		var lassoSvgAttrs = isLassoing ? _List_fromArray(
			[
				A2(
				$elm$html$Html$Events$on,
				'mousemove',
				decodeLassoCoords($author$project$Main$LassoMove)),
				A2(
				$elm$html$Html$Events$on,
				'mouseup',
				$elm$json$Json$Decode$succeed($author$project$Main$LassoEnd)),
				A2(
				$elm$html$Html$Events$on,
				'mouseleave',
				$elm$json$Json$Decode$succeed($author$project$Main$LassoEnd))
			]) : _List_Nil;
		var cw = response.aq.l;
		var w = $elm$core$String$fromFloat(cw);
		var compositeOverlays = showComposite ? A2($elm$core$List$map, $author$project$Main$viewBrickOverlay, response.E) : _List_Nil;
		var ch = response.aq.j;
		var gridLayer = ((!model.m) && (model.aN || model.aZ)) ? A4(
			$author$project$Main$viewGrid,
			cw,
			ch,
			A2($author$project$Main$waveColor, model.ah, 1.0),
			model._) : _List_Nil;
		var h = $elm$core$String$fromFloat(ch);
		var lightsLayer = function () {
			var _v1 = _Utils_Tuple2(model.aP, response.a2);
			if (_v1.a && (!_v1.b.$)) {
				var url = _v1.b.a;
				return _List_fromArray(
					[
						A2(
						$elm$svg$Svg$image,
						_List_fromArray(
							[
								$elm$svg$Svg$Attributes$x('0'),
								$elm$svg$Svg$Attributes$y('0'),
								$elm$svg$Svg$Attributes$width(w),
								$elm$svg$Svg$Attributes$height(h),
								A2($elm$html$Html$Attributes$attribute, 'href', url),
								$elm$svg$Svg$Attributes$style('pointer-events: none;')
							]),
						_List_Nil)
					]);
			} else {
				return _List_Nil;
			}
		}();
		var outlinesPngLayer = ((!model.m) && (!isGenerated)) ? _List_fromArray(
			[
				A2(
				$elm$svg$Svg$image,
				_List_fromArray(
					[
						$elm$svg$Svg$Attributes$x('0'),
						$elm$svg$Svg$Attributes$y('0'),
						$elm$svg$Svg$Attributes$width(w),
						$elm$svg$Svg$Attributes$height(h),
						A2($elm$html$Html$Attributes$attribute, 'href', response.bo),
						$elm$svg$Svg$Attributes$style('pointer-events: none;')
					]),
				_List_Nil)
			]) : _List_Nil;
		var lassoBackdrop = ((!model.m) && (isGenerated && (!_Utils_eq(model.k, $elm$core$Maybe$Nothing)))) ? _List_fromArray(
			[
				A2(
				$elm$svg$Svg$rect,
				_List_fromArray(
					[
						$elm$svg$Svg$Attributes$x('-200'),
						$elm$svg$Svg$Attributes$y('-10'),
						$elm$svg$Svg$Attributes$width(
						$elm$core$String$fromFloat(cw + 400)),
						$elm$svg$Svg$Attributes$height(
						$elm$core$String$fromFloat(ch + 20)),
						$elm$svg$Svg$Attributes$fill('transparent'),
						$elm$svg$Svg$Attributes$style('cursor: crosshair;'),
						A2(
						$elm$html$Html$Events$on,
						'mousedown',
						decodeLassoCoords($author$project$Main$LassoStart))
					]),
				_List_Nil)
			]) : _List_Nil;
		var blueprintLayer = ((!model.m) && isGenerated) ? A2($elm$core$List$map, $author$project$Main$viewPieceBlueprintPath, model.d) : _List_Nil;
		var bgImageLayer = function () {
			var _v0 = response.bc;
			if (!_v0.$) {
				var url = _v0.a;
				return ((model.e === 3) || (model.e === 5)) ? _List_fromArray(
					[
						A2(
						$elm$svg$Svg$image,
						_List_fromArray(
							[
								$elm$svg$Svg$Attributes$x('0'),
								$elm$svg$Svg$Attributes$y('0'),
								$elm$svg$Svg$Attributes$width(w),
								$elm$svg$Svg$Attributes$height(h),
								A2($elm$html$Html$Attributes$attribute, 'href', url),
								$elm$svg$Svg$Attributes$style('pointer-events: none;')
							]),
						_List_Nil)
					]) : _List_Nil;
			} else {
				return _List_Nil;
			}
		}();
		var baseLayer = model.m ? (response.a0 ? _List_fromArray(
			[
				A2(
				$elm$svg$Svg$image,
				_List_fromArray(
					[
						$elm$svg$Svg$Attributes$x('0'),
						$elm$svg$Svg$Attributes$y('0'),
						$elm$svg$Svg$Attributes$width(w),
						$elm$svg$Svg$Attributes$height(h),
						A2($elm$html$Html$Attributes$attribute, 'href', response.a$)
					]),
				_List_Nil)
			]) : _List_Nil) : (showPieceImages ? A2(
			$elm$core$List$map,
			$author$project$Main$viewPieceImage(model.s),
			visiblePieces) : (showComposite ? _List_fromArray(
			[
				A2(
				$elm$svg$Svg$image,
				_List_fromArray(
					[
						$elm$svg$Svg$Attributes$x('0'),
						$elm$svg$Svg$Attributes$y('0'),
						$elm$svg$Svg$Attributes$width(w),
						$elm$svg$Svg$Attributes$height(h),
						A2($elm$html$Html$Attributes$attribute, 'href', response.a$)
					]),
				_List_Nil)
			]) : _List_Nil));
		return A2(
			$elm$svg$Svg$svg,
			_Utils_ap(
				_List_fromArray(
					[
						$elm$svg$Svg$Attributes$viewBox(
						'-200 -10 ' + ($elm$core$String$fromFloat(cw + 400) + (' ' + $elm$core$String$fromFloat(ch + 20)))),
						$elm$svg$Svg$Attributes$class('house-svg'),
						$elm$svg$Svg$Attributes$width(
						$elm$core$String$fromFloat((cw + 400) * effectiveScale)),
						$elm$svg$Svg$Attributes$height(
						$elm$core$String$fromFloat((ch + 20) * effectiveScale))
					]),
				lassoSvgAttrs),
			model.m ? _List_fromArray(
				[
					A2($elm$svg$Svg$g, _List_Nil, baseLayer),
					A2($elm$svg$Svg$g, _List_Nil, editOverlays)
				]) : _List_fromArray(
				[
					A2($elm$svg$Svg$g, _List_Nil, bgImageLayer),
					A2($elm$svg$Svg$g, _List_Nil, blueprintLayer),
					A2($elm$svg$Svg$g, _List_Nil, baseLayer),
					A2($elm$svg$Svg$g, _List_Nil, lightsLayer),
					A2($elm$svg$Svg$g, _List_Nil, compositeOverlays),
					A2($elm$svg$Svg$g, _List_Nil, gridLayer),
					A2($elm$svg$Svg$g, _List_Nil, lassoBackdrop),
					A2($elm$svg$Svg$g, _List_Nil, pieceOverlays),
					A2($elm$svg$Svg$g, _List_Nil, outlineLayer),
					A2($elm$svg$Svg$g, _List_Nil, outlinesPngLayer),
					A2($elm$svg$Svg$g, _List_Nil, numberLabels),
					A2($elm$svg$Svg$g, _List_Nil, lassoRect)
				]));
	});
var $author$project$Main$SetZoomGridActive = function (a) {
	return {$: 62, a: a};
};
var $author$project$Main$SetZoomLevel = function (a) {
	return {$: 61, a: a};
};
var $elm$html$Html$button = _VirtualDom_node('button');
var $elm$html$Html$input = _VirtualDom_node('input');
var $elm$html$Html$Attributes$list = _VirtualDom_attribute('list');
var $elm$html$Html$Attributes$max = $elm$html$Html$Attributes$stringProperty('max');
var $elm$html$Html$Attributes$min = $elm$html$Html$Attributes$stringProperty('min');
var $elm$virtual_dom$VirtualDom$node = function (tag) {
	return _VirtualDom_node(
		_VirtualDom_noScript(tag));
};
var $elm$html$Html$node = $elm$virtual_dom$VirtualDom$node;
var $elm$html$Html$Events$alwaysStop = function (x) {
	return _Utils_Tuple2(x, true);
};
var $elm$virtual_dom$VirtualDom$MayStopPropagation = function (a) {
	return {$: 1, a: a};
};
var $elm$html$Html$Events$stopPropagationOn = F2(
	function (event, decoder) {
		return A2(
			$elm$virtual_dom$VirtualDom$on,
			event,
			$elm$virtual_dom$VirtualDom$MayStopPropagation(decoder));
	});
var $elm$json$Json$Decode$at = F2(
	function (fields, decoder) {
		return A3($elm$core$List$foldr, $elm$json$Json$Decode$field, decoder, fields);
	});
var $elm$html$Html$Events$targetValue = A2(
	$elm$json$Json$Decode$at,
	_List_fromArray(
		['target', 'value']),
	$elm$json$Json$Decode$string);
var $elm$html$Html$Events$onInput = function (tagger) {
	return A2(
		$elm$html$Html$Events$stopPropagationOn,
		'input',
		A2(
			$elm$json$Json$Decode$map,
			$elm$html$Html$Events$alwaysStop,
			A2($elm$json$Json$Decode$map, tagger, $elm$html$Html$Events$targetValue)));
};
var $elm$html$Html$option = _VirtualDom_node('option');
var $elm$html$Html$span = _VirtualDom_node('span');
var $elm$html$Html$Attributes$step = function (n) {
	return A2($elm$html$Html$Attributes$stringProperty, 'step', n);
};
var $elm$html$Html$Attributes$type_ = $elm$html$Html$Attributes$stringProperty('type');
var $elm$html$Html$Attributes$value = $elm$html$Html$Attributes$stringProperty('value');
var $author$project$Main$viewZoomSlider = function (model) {
	var pct = $elm$core$Basics$round(model.aE * 100);
	var label = $elm$core$String$fromInt(pct) + '%';
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('zoom-slider-bar')
			]),
		_List_fromArray(
			[
				A2(
				$elm$html$Html$span,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('zoom-icon')
					]),
				_List_fromArray(
					[
						$elm$html$Html$text('+')
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('zoom-slider-wrap')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$input,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$type_('range'),
								$elm$html$Html$Attributes$class('zoom-slider'),
								$elm$html$Html$Attributes$list('zoom-ticks'),
								$elm$html$Html$Attributes$min('0.25'),
								$elm$html$Html$Attributes$max('4.0'),
								$elm$html$Html$Attributes$step('0.05'),
								$elm$html$Html$Attributes$value(
								$elm$core$String$fromFloat(model.aE)),
								$elm$html$Html$Events$onInput(
								function (s) {
									return A2(
										$elm$core$Maybe$withDefault,
										$author$project$Main$NoOp,
										A2(
											$elm$core$Maybe$map,
											$author$project$Main$SetZoomLevel,
											$elm$core$String$toFloat(s)));
								}),
								$elm$html$Html$Events$onMouseEnter(
								$author$project$Main$SetZoomGridActive(true)),
								$elm$html$Html$Events$onMouseLeave(
								$author$project$Main$SetZoomGridActive(false))
							]),
						_List_Nil),
						A3(
						$elm$html$Html$node,
						'datalist',
						_List_fromArray(
							[
								$elm$html$Html$Attributes$id('zoom-ticks')
							]),
						_List_fromArray(
							[
								A2(
								$elm$html$Html$option,
								_List_fromArray(
									[
										$elm$html$Html$Attributes$value('1')
									]),
								_List_Nil)
							])),
						A2(
						$elm$html$Html$button,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('zoom-notch-label'),
								$elm$html$Html$Events$onClick(
								$author$project$Main$SetZoomLevel(1.0))
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('100%')
							]))
					])),
				A2(
				$elm$html$Html$span,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('zoom-icon')
					]),
				_List_fromArray(
					[
						$elm$html$Html$text('−')
					])),
				A2(
				$elm$html$Html$span,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('zoom-val')
					]),
				_List_fromArray(
					[
						$elm$html$Html$text(label)
					]))
			]));
};
var $author$project$Main$viewCanvasCol = F2(
	function (model, response) {
		return A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$class('canvas-col')
				]),
			_List_fromArray(
				[
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('canvas-house-wrap')
						]),
					_List_fromArray(
						[
							A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('canvas-area'),
									$elm$html$Html$Attributes$id('house-scroll')
								]),
							_List_fromArray(
								[
									A2(
									$elm$html$Html$div,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('canvas-spacer')
										]),
									_List_Nil),
									A2($author$project$Main$viewMainSvg, response, model),
									model.x ? A2(
									$elm$html$Html$div,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('canvas-spinner-overlay')
										]),
									_List_fromArray(
										[
											A2(
											$elm$html$Html$div,
											_List_fromArray(
												[
													$elm$html$Html$Attributes$class('canvas-spinner')
												]),
											_List_Nil)
										])) : $elm$html$Html$text('')
								])),
							$author$project$Main$viewZoomSlider(model)
						]))
				]));
	});
var $author$project$Main$LoadFile = function (a) {
	return {$: 4, a: a};
};
var $author$project$Main$PickFile = {$: 1};
var $elm$json$Json$Encode$bool = _Json_wrap;
var $elm$html$Html$Attributes$boolProperty = F2(
	function (key, bool) {
		return A2(
			_VirtualDom_property,
			key,
			$elm$json$Json$Encode$bool(bool));
	});
var $elm$html$Html$Attributes$disabled = $elm$html$Html$Attributes$boolProperty('disabled');
var $author$project$Main$viewFileList = function (model) {
	var isBusy = _Utils_eq(model.o, $author$project$Main$Loading);
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('file-list')
			]),
		_Utils_ap(
			_List_fromArray(
				[
					A2(
					$elm$html$Html$button,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('file-entry file-entry-browse'),
							$elm$html$Html$Events$onClick($author$project$Main$PickFile),
							$elm$html$Html$Attributes$disabled(isBusy)
						]),
					_List_fromArray(
						[
							$elm$html$Html$text('Browse…')
						]))
				]),
			$elm$core$List$isEmpty(model.aM) ? _List_fromArray(
				[
					A2(
					$elm$html$Html$span,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('file-list-empty')
						]),
					_List_fromArray(
						[
							$elm$html$Html$text('No files in in/')
						]))
				]) : A2(
				$elm$core$List$map,
				function (f) {
					return A2(
						$elm$html$Html$button,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('file-entry'),
								$elm$html$Html$Events$onClick(
								$author$project$Main$LoadFile(f.bp)),
								$elm$html$Html$Attributes$disabled(isBusy)
							]),
						_List_fromArray(
							[
								$elm$html$Html$text(f.N)
							]));
				},
				model.aM)));
};
var $author$project$Main$ToggleGrid = function (a) {
	return {$: 15, a: a};
};
var $elm$html$Html$Attributes$checked = $elm$html$Html$Attributes$boolProperty('checked');
var $elm$html$Html$Attributes$for = $elm$html$Html$Attributes$stringProperty('htmlFor');
var $elm$html$Html$label = _VirtualDom_node('label');
var $elm$html$Html$Events$targetChecked = A2(
	$elm$json$Json$Decode$at,
	_List_fromArray(
		['target', 'checked']),
	$elm$json$Json$Decode$bool);
var $elm$html$Html$Events$onCheck = function (tagger) {
	return A2(
		$elm$html$Html$Events$on,
		'change',
		A2($elm$json$Json$Decode$map, tagger, $elm$html$Html$Events$targetChecked));
};
var $author$project$Main$StartColorPick = F3(
	function (a, b, c) {
		return {$: 64, a: a, b: b, c: c};
	});
var $elm$virtual_dom$VirtualDom$style = _VirtualDom_style;
var $elm$html$Html$Attributes$style = $elm$virtual_dom$VirtualDom$style;
var $elm$html$Html$Attributes$title = $elm$html$Html$Attributes$stringProperty('title');
var $author$project$Main$viewGridColorSwatch = function (model) {
	return A2(
		$elm$html$Html$span,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('wave-swatch wave-swatch-sm'),
				A2(
				$elm$html$Html$Attributes$style,
				'background-color',
				A2($author$project$Main$waveColor, model.ah, 1.0)),
				A2(
				$elm$html$Html$Events$stopPropagationOn,
				'mousedown',
				A3(
					$elm$json$Json$Decode$map2,
					F2(
						function (mx, my) {
							return _Utils_Tuple2(
								A3($author$project$Main$StartColorPick, $author$project$Main$GridColorTarget, mx, my),
								true);
						}),
					A2($elm$json$Json$Decode$field, 'clientX', $elm$json$Json$Decode$float),
					A2($elm$json$Json$Decode$field, 'clientY', $elm$json$Json$Decode$float))),
				$elm$html$Html$Attributes$title('Pick grid color')
			]),
		_List_Nil);
};
var $author$project$Main$viewCheckboxGrid = function (model) {
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('checkbox-group')
			]),
		_List_fromArray(
			[
				A2(
				$elm$html$Html$input,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$type_('checkbox'),
						$elm$html$Html$Attributes$id('cbGrid'),
						$elm$html$Html$Attributes$checked(model.aN),
						$elm$html$Html$Events$onCheck($author$project$Main$ToggleGrid)
					]),
				_List_Nil),
				A2(
				$elm$html$Html$label,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$for('cbGrid')
					]),
				_List_fromArray(
					[
						$elm$html$Html$text('Show grid')
					])),
				$author$project$Main$viewGridColorSwatch(model)
			]));
};
var $author$project$Main$ToggleLights = function (a) {
	return {$: 17, a: a};
};
var $author$project$Main$viewCheckboxLights = function (model) {
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('checkbox-group')
			]),
		_List_fromArray(
			[
				A2(
				$elm$html$Html$input,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$type_('checkbox'),
						$elm$html$Html$Attributes$id('cbLights'),
						$elm$html$Html$Attributes$checked(model.aP),
						$elm$html$Html$Events$onCheck($author$project$Main$ToggleLights)
					]),
				_List_Nil),
				A2(
				$elm$html$Html$label,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$for('cbLights')
					]),
				_List_fromArray(
					[
						$elm$html$Html$text('Show lights')
					]))
			]));
};
var $elm$html$Html$h3 = _VirtualDom_node('h3');
var $author$project$Main$viewSectionTitle = function (title) {
	return A2(
		$elm$html$Html$h3,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('section-title')
			]),
		_List_fromArray(
			[
				$elm$html$Html$text(title)
			]));
};
var $author$project$Main$viewTogglesBox = function (children) {
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('toggles-box')
			]),
		children);
};
var $author$project$Main$viewBlueprintTools = function (model) {
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('tools-pane')
			]),
		_List_fromArray(
			[
				$author$project$Main$viewTogglesBox(
				_List_fromArray(
					[
						$author$project$Main$viewCheckboxLights(model),
						$author$project$Main$viewCheckboxGrid(model)
					])),
				$author$project$Main$viewSectionTitle('Blueprint')
			]));
};
var $author$project$Main$RequestExport = {$: 40};
var $author$project$Main$SetExportHouseName = function (a) {
	return {$: 37, a: a};
};
var $author$project$Main$SetExportLocation = function (a) {
	return {$: 36, a: a};
};
var $author$project$Main$SetExportPosition = function (a) {
	return {$: 38, a: a};
};
var $author$project$Main$SetExportSpacing = function (a) {
	return {$: 39, a: a};
};
var $author$project$Main$locations = _List_fromArray(
	['Tutorial', 'Rome', 'Athens', 'Amsterdam', 'Paris', 'Palermo', 'Venice', 'Frankfurt', 'New York', 'Prague']);
var $elm$html$Html$select = _VirtualDom_node('select');
var $elm$html$Html$Attributes$selected = $elm$html$Html$Attributes$boolProperty('selected');
var $author$project$Main$ToggleNumbers = function (a) {
	return {$: 16, a: a};
};
var $author$project$Main$viewCheckboxNumbers = function (model) {
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('checkbox-group')
			]),
		_List_fromArray(
			[
				A2(
				$elm$html$Html$input,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$type_('checkbox'),
						$elm$html$Html$Attributes$id('cbNumbers'),
						$elm$html$Html$Attributes$checked(model.aw),
						$elm$html$Html$Events$onCheck($author$project$Main$ToggleNumbers)
					]),
				_List_Nil),
				A2(
				$elm$html$Html$label,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$for('cbNumbers')
					]),
				_List_fromArray(
					[
						$elm$html$Html$text('Show position numbers')
					]))
			]));
};
var $author$project$Main$ToggleOutlines = function (a) {
	return {$: 14, a: a};
};
var $author$project$Main$viewCheckboxOutlines = function (model) {
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('checkbox-group')
			]),
		_List_fromArray(
			[
				A2(
				$elm$html$Html$input,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$type_('checkbox'),
						$elm$html$Html$Attributes$id('cbOutlines'),
						$elm$html$Html$Attributes$checked(model.aQ),
						$elm$html$Html$Events$onCheck($author$project$Main$ToggleOutlines)
					]),
				_List_Nil),
				A2(
				$elm$html$Html$label,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$for('cbOutlines')
					]),
				_List_fromArray(
					[
						$elm$html$Html$text('Show piece outlines')
					])),
				A2(
				$elm$html$Html$span,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('wave-swatch wave-swatch-sm'),
						A2(
						$elm$html$Html$Attributes$style,
						'background-color',
						A2($author$project$Main$waveColor, model.aj, 1.0)),
						A2(
						$elm$html$Html$Events$stopPropagationOn,
						'mousedown',
						A3(
							$elm$json$Json$Decode$map2,
							F2(
								function (mx, my) {
									return _Utils_Tuple2(
										A3($author$project$Main$StartColorPick, $author$project$Main$OutlineColorTarget, mx, my),
										true);
								}),
							A2($elm$json$Json$Decode$field, 'clientX', $elm$json$Json$Decode$float),
							A2($elm$json$Json$Decode$field, 'clientY', $elm$json$Json$Decode$float))),
						$elm$html$Html$Attributes$title('Pick outline color')
					]),
				_List_Nil)
			]));
};
var $author$project$Main$ToggleWaveOverlay = function (a) {
	return {$: 20, a: a};
};
var $author$project$Main$viewCheckboxWaveOverlay = function (model) {
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('checkbox-group')
			]),
		_List_fromArray(
			[
				A2(
				$elm$html$Html$input,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$type_('checkbox'),
						$elm$html$Html$Attributes$id('cbWaveOverlay'),
						$elm$html$Html$Attributes$checked(model.ax),
						$elm$html$Html$Events$onCheck($author$project$Main$ToggleWaveOverlay)
					]),
				_List_Nil),
				A2(
				$elm$html$Html$label,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$for('cbWaveOverlay')
					]),
				_List_fromArray(
					[
						$elm$html$Html$text('Show wave overlays')
					]))
			]));
};
var $author$project$Main$viewExportTools = function (model) {
	var assignedIds = A2(
		$elm$core$List$concatMap,
		function ($) {
			return $.b;
		},
		model.c);
	var hasUnassigned = A2(
		$elm$core$List$any,
		function (p) {
			return !A2($elm$core$List$member, p.a, assignedIds);
		},
		model.d);
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('tools-pane')
			]),
		_List_fromArray(
			[
				$author$project$Main$viewTogglesBox(
				_List_fromArray(
					[
						$author$project$Main$viewCheckboxLights(model),
						$author$project$Main$viewCheckboxGrid(model),
						$author$project$Main$viewCheckboxOutlines(model),
						$author$project$Main$viewCheckboxWaveOverlay(model),
						$author$project$Main$viewCheckboxNumbers(model)
					])),
				$author$project$Main$viewSectionTitle('Export'),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('field-row')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$label,
						_List_Nil,
						_List_fromArray(
							[
								$elm$html$Html$text('Location')
							])),
						A2(
						$elm$html$Html$select,
						_List_fromArray(
							[
								$elm$html$Html$Events$onInput($author$project$Main$SetExportLocation)
							]),
						A2(
							$elm$core$List$map,
							function (loc) {
								return A2(
									$elm$html$Html$option,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$value(loc),
											$elm$html$Html$Attributes$selected(
											_Utils_eq(loc, model.aI))
										]),
									_List_fromArray(
										[
											$elm$html$Html$text(loc)
										]));
							},
							$author$project$Main$locations))
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('field-row')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$label,
						_List_Nil,
						_List_fromArray(
							[
								$elm$html$Html$text('House name')
							])),
						A2(
						$elm$html$Html$input,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$type_('text'),
								$elm$html$Html$Attributes$value(model.af),
								$elm$html$Html$Events$onInput($author$project$Main$SetExportHouseName)
							]),
						_List_Nil)
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('field-row')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$label,
						_List_Nil,
						_List_fromArray(
							[
								$elm$html$Html$text('Position in location')
							])),
						A2(
						$elm$html$Html$input,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$type_('number'),
								$elm$html$Html$Attributes$value(model.aJ),
								$elm$html$Html$Events$onInput($author$project$Main$SetExportPosition),
								$elm$html$Html$Attributes$min('0'),
								$elm$html$Html$Attributes$step('1')
							]),
						_List_Nil)
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('field-row')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$label,
						_List_Nil,
						_List_fromArray(
							[
								$elm$html$Html$text('Spacing (units)')
							])),
						A2(
						$elm$html$Html$input,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$type_('number'),
								$elm$html$Html$Attributes$value(model.aK),
								$elm$html$Html$Events$onInput($author$project$Main$SetExportSpacing),
								$elm$html$Html$Attributes$min('0'),
								$elm$html$Html$Attributes$step('0.5')
							]),
						_List_Nil)
					])),
				A2(
				$elm$html$Html$button,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('primary'),
						$elm$html$Html$Events$onClick($author$project$Main$RequestExport),
						$elm$html$Html$Attributes$disabled(hasUnassigned || model.S),
						$elm$html$Html$Attributes$title(
						hasUnassigned ? 'All pieces must be assigned to waves before exporting' : '')
					]),
				_List_fromArray(
					[
						$elm$html$Html$text(
						model.S ? 'Exporting\u2026' : 'Export ZIP')
					]))
			]));
};
var $author$project$Main$RequestGenerate = {$: 10};
var $author$project$Main$SetMinBorder = function (a) {
	return {$: 8, a: a};
};
var $author$project$Main$SetTargetCount = function (a) {
	return {$: 7, a: a};
};
var $elm$html$Html$h2 = _VirtualDom_node('h2');
var $author$project$Main$viewImportStats = function (response) {
	var totalBricks = $elm$core$List$length(response.E);
	var skipped = $elm$core$List$length(
		A2(
			$elm$core$List$filter,
			$elm$core$String$startsWith('SKIPPED:'),
			response.aY));
	var realWarnings = A2(
		$elm$core$List$map,
		A2($elm$core$String$replace, 'MULTI_OBJECT: ', ''),
		A2(
			$elm$core$List$filter,
			$elm$core$String$startsWith('MULTI_OBJECT:'),
			response.aY));
	var covered = $elm$core$List$length(
		A2(
			$elm$core$List$filter,
			$elm$core$String$startsWith('COVERED:'),
			response.aY));
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('stats')
			]),
		_Utils_ap(
			_List_fromArray(
				[
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('row')
						]),
					_List_fromArray(
						[
							$elm$html$Html$text('Bricks imported'),
							A2(
							$elm$html$Html$span,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('val')
								]),
							_List_fromArray(
								[
									$elm$html$Html$text(
									$elm$core$String$fromInt(totalBricks))
								]))
						]))
				]),
			_Utils_ap(
				(skipped > 0) ? _List_fromArray(
					[
						A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('row')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Skipped (no polygon)'),
								A2(
								$elm$html$Html$span,
								_List_fromArray(
									[
										$elm$html$Html$Attributes$class('val')
									]),
								_List_fromArray(
									[
										$elm$html$Html$text(
										$elm$core$String$fromInt(skipped))
									]))
							]))
					]) : _List_Nil,
				_Utils_ap(
					(covered > 0) ? _List_fromArray(
						[
							A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('row')
								]),
							_List_fromArray(
								[
									$elm$html$Html$text('Covered (hidden)'),
									A2(
									$elm$html$Html$span,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('val')
										]),
									_List_fromArray(
										[
											$elm$html$Html$text(
											$elm$core$String$fromInt(covered))
										]))
								]))
						]) : _List_Nil,
					(!$elm$core$List$isEmpty(realWarnings)) ? _Utils_ap(
						_List_fromArray(
							[
								A2(
								$elm$html$Html$div,
								_List_fromArray(
									[
										$elm$html$Html$Attributes$class('row'),
										A2($elm$html$Html$Attributes$style, 'margin-top', '4px')
									]),
								_List_fromArray(
									[
										$elm$html$Html$text('Warnings:')
									]))
							]),
						A2(
							$elm$core$List$map,
							function (w) {
								return A2(
									$elm$html$Html$div,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('row'),
											A2($elm$html$Html$Attributes$style, 'color', '#b04020'),
											A2($elm$html$Html$Attributes$style, 'font-size', '10px')
										]),
									_List_fromArray(
										[
											$elm$html$Html$text(w)
										]));
							},
							realWarnings)) : _List_Nil))));
};
var $author$project$Main$viewStats = function (model) {
	var pieceCount = (model.y === 2) ? $elm$core$String$fromInt(
		$elm$core$List$length(model.d)) : '-';
	var canvasInfo = function () {
		var _v1 = model.o;
		if (_v1.$ === 2) {
			var r = _v1.a;
			return $elm$core$String$fromFloat(r.aq.l) + ('\u00D7' + $elm$core$String$fromFloat(r.aq.j));
		} else {
			return '-';
		}
	}();
	var brickCount = function () {
		var _v0 = model.o;
		if (_v0.$ === 2) {
			var r = _v0.a;
			return $elm$core$String$fromInt(
				$elm$core$List$length(r.E));
		} else {
			return '-';
		}
	}();
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('stats')
			]),
		_List_fromArray(
			[
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('row')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$span,
						_List_Nil,
						_List_fromArray(
							[
								$elm$html$Html$text('Canvas')
							])),
						A2(
						$elm$html$Html$span,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('val')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text(canvasInfo)
							]))
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('row')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$span,
						_List_Nil,
						_List_fromArray(
							[
								$elm$html$Html$text('Total Bricks')
							])),
						A2(
						$elm$html$Html$span,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('val')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text(brickCount)
							]))
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('row')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$span,
						_List_Nil,
						_List_fromArray(
							[
								$elm$html$Html$text('Puzzle Pieces')
							])),
						A2(
						$elm$html$Html$span,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('val')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text(pieceCount)
							]))
					]))
			]));
};
var $author$project$Main$viewStatusBadge = function (model) {
	var _v0 = model.o;
	switch (_v0.$) {
		case 0:
			return $elm$html$Html$text('');
		case 1:
			return A2(
				$elm$html$Html$span,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('status loading')
					]),
				_List_fromArray(
					[
						$elm$html$Html$text('Parsing PDF\u2026')
					]));
		case 2:
			return $elm$html$Html$text('');
		default:
			var err = _v0.a;
			return A2(
				$elm$html$Html$span,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('status error')
					]),
				_List_fromArray(
					[
						$elm$html$Html$text('Error: ' + err)
					]));
	}
};
var $author$project$Main$viewGenerateTools = F2(
	function (model, response) {
		var isLoaded = function () {
			var _v0 = model.o;
			if (_v0.$ === 2) {
				return true;
			} else {
				return false;
			}
		}();
		var isGenerating = model.y === 1;
		var isBusy = _Utils_eq(model.o, $author$project$Main$Loading) || (model.x || model.S);
		var hasLights = !_Utils_eq(response.a2, $elm$core$Maybe$Nothing);
		return A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$class('tools-pane')
				]),
			_List_fromArray(
				[
					$author$project$Main$viewTogglesBox(
					_List_fromArray(
						[
							$author$project$Main$viewCheckboxLights(model),
							$author$project$Main$viewCheckboxGrid(model)
						])),
					$author$project$Main$viewStatusBadge(model),
					$author$project$Main$viewSectionTitle('Import'),
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('param-group')
						]),
					_List_fromArray(
						[
							A2(
							$elm$html$Html$label,
							_List_Nil,
							_List_fromArray(
								[
									$elm$html$Html$text('Target Pieces '),
									A2(
									$elm$html$Html$span,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('value')
										]),
									_List_fromArray(
										[
											$elm$html$Html$text(
											$elm$core$String$fromInt(model.az))
										]))
								])),
							A2(
							$elm$html$Html$input,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$type_('range'),
									$elm$html$Html$Attributes$min('5'),
									$elm$html$Html$Attributes$max('181'),
									$elm$html$Html$Attributes$value(
									$elm$core$String$fromInt(model.az)),
									$elm$html$Html$Events$onInput($author$project$Main$SetTargetCount)
								]),
							_List_Nil)
						])),
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('param-group')
						]),
					_List_fromArray(
						[
							A2(
							$elm$html$Html$label,
							_List_Nil,
							_List_fromArray(
								[
									$elm$html$Html$text('Min. Common Border Length '),
									A2(
									$elm$html$Html$span,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('value')
										]),
									_List_fromArray(
										[
											$elm$html$Html$text(
											$elm$core$String$fromInt(model.at))
										])),
									$elm$html$Html$text('px')
								])),
							A2(
							$elm$html$Html$input,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$type_('range'),
									$elm$html$Html$Attributes$min('0'),
									$elm$html$Html$Attributes$max('50'),
									$elm$html$Html$Attributes$value(
									$elm$core$String$fromInt(model.at)),
									$elm$html$Html$Events$onInput($author$project$Main$SetMinBorder)
								]),
							_List_Nil)
						])),
					A2(
					$elm$html$Html$h2,
					_List_Nil,
					_List_fromArray(
						[
							$elm$html$Html$text('Import')
						])),
					$author$project$Main$viewImportStats(response),
					A2(
					$elm$html$Html$h2,
					_List_Nil,
					_List_fromArray(
						[
							$elm$html$Html$text('Puzzle')
						])),
					$author$project$Main$viewStats(model),
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('tools-divider')
						]),
					_List_Nil),
					A2(
					$elm$html$Html$button,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('primary'),
							$elm$html$Html$Attributes$disabled((!isLoaded) || (isBusy || isGenerating)),
							$elm$html$Html$Events$onClick($author$project$Main$RequestGenerate)
						]),
					_List_fromArray(
						[
							$elm$html$Html$text(
							isGenerating ? 'Generating\u2026' : 'Generate Puzzle')
						]))
				]));
	});
var $author$project$Main$AddGroup = {$: 50};
var $author$project$Main$ToggleGroupOverlay = function (a) {
	return {$: 19, a: a};
};
var $author$project$Main$viewCheckboxGroupOverlay = function (model) {
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('checkbox-group')
			]),
		_List_fromArray(
			[
				A2(
				$elm$html$Html$input,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$type_('checkbox'),
						$elm$html$Html$Attributes$id('cbGroupOverlay'),
						$elm$html$Html$Attributes$checked(model.aO),
						$elm$html$Html$Events$onCheck($author$project$Main$ToggleGroupOverlay)
					]),
				_List_Nil),
				A2(
				$elm$html$Html$label,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$for('cbGroupOverlay')
					]),
				_List_fromArray(
					[
						$elm$html$Html$text('Show overlay')
					]))
			]));
};
var $author$project$Main$DragEnterGroup = function (a) {
	return {$: 55, a: a};
};
var $author$project$Main$DropOnGroup = function (a) {
	return {$: 56, a: a};
};
var $author$project$Main$GroupColorTarget = function (a) {
	return {$: 1, a: a};
};
var $author$project$Main$RemoveGroup = function (a) {
	return {$: 52, a: a};
};
var $author$project$Main$SelectGroup = function (a) {
	return {$: 51, a: a};
};
var $author$project$Main$ToggleGroupLock = function (a) {
	return {$: 49, a: a};
};
var $elm$core$Tuple$second = function (_v0) {
	var y = _v0.b;
	return y;
};
var $elm$html$Html$Attributes$classList = function (classes) {
	return $elm$html$Html$Attributes$class(
		A2(
			$elm$core$String$join,
			' ',
			A2(
				$elm$core$List$map,
				$elm$core$Tuple$first,
				A2($elm$core$List$filter, $elm$core$Tuple$second, classes))));
};
var $elm$svg$Svg$Attributes$d = _VirtualDom_attribute('d');
var $elm$svg$Svg$path = $elm$svg$Svg$trustedNode('path');
var $author$project$Main$iconLockClosed = A2(
	$elm$svg$Svg$svg,
	_List_fromArray(
		[
			$elm$svg$Svg$Attributes$viewBox('0 0 24 24'),
			$elm$svg$Svg$Attributes$width('14'),
			$elm$svg$Svg$Attributes$height('14'),
			$elm$svg$Svg$Attributes$fill('currentColor')
		]),
	_List_fromArray(
		[
			A2(
			$elm$svg$Svg$path,
			_List_fromArray(
				[
					$elm$svg$Svg$Attributes$d('M6 22q-.825 0-1.412-.587T4 20V10q0-.825.588-1.412T6 8h1V6q0-2.075 1.463-3.537T12 1t3.538 1.463T17 6v2h1q.825 0 1.413.588T20 10v10q0 .825-.587 1.413T18 22zm0-2h12V10H6zm7.413-3.588Q14 15.826 14 15t-.587-1.412T12 13t-1.412.588T10 15t.588 1.413T12 17t1.413-.587M9 8h6V6q0-1.25-.875-2.125T12 3t-2.125.875T9 6zM6 20V10z')
				]),
			_List_Nil)
		]));
var $author$project$Main$iconLockOpen = A2(
	$elm$svg$Svg$svg,
	_List_fromArray(
		[
			$elm$svg$Svg$Attributes$viewBox('0 0 24 24'),
			$elm$svg$Svg$Attributes$width('14'),
			$elm$svg$Svg$Attributes$height('14'),
			$elm$svg$Svg$Attributes$fill('currentColor')
		]),
	_List_fromArray(
		[
			A2(
			$elm$svg$Svg$path,
			_List_fromArray(
				[
					$elm$svg$Svg$Attributes$d('M6 20h12V10H6zm7.413-3.588Q14 15.826 14 15t-.587-1.412T12 13t-1.412.588T10 15t.588 1.413T12 17t1.413-.587M6 20V10zm0 2q-.825 0-1.412-.587T4 20V10q0-.825.588-1.412T6 8h7V6q0-2.075 1.463-3.537T18 1t3.538 1.463T23 6h-2q0-1.25-.875-2.125T18 3t-2.125.875T15 6v2h3q.825 0 1.413.588T20 10v10q0 .825-.587 1.413T18 22z')
				]),
			_List_Nil)
		]));
var $elm$virtual_dom$VirtualDom$MayPreventDefault = function (a) {
	return {$: 2, a: a};
};
var $elm$html$Html$Events$preventDefaultOn = F2(
	function (event, decoder) {
		return A2(
			$elm$virtual_dom$VirtualDom$on,
			event,
			$elm$virtual_dom$VirtualDom$MayPreventDefault(decoder));
	});
var $author$project$Main$DragEnterPiece = function (a) {
	return {$: 46, a: a};
};
var $author$project$Main$DragPieceEnd = {$: 44};
var $author$project$Main$DragPieceStart = function (a) {
	return {$: 43, a: a};
};
var $author$project$Main$RemovePieceFromWave = F2(
	function (a, b) {
		return {$: 27, a: a, b: b};
	});
var $elm$html$Html$img = _VirtualDom_node('img');
var $elm$html$Html$Attributes$src = function (url) {
	return A2(
		$elm$html$Html$Attributes$stringProperty,
		'src',
		_VirtualDom_noJavaScriptOrHtmlUri(url));
};
var $author$project$Main$viewPieceThumb = F6(
	function (removeInfo, isLocked, hoveredId, pieceId, dataUrl, maybePos) {
		var isHovered = _Utils_eq(
			hoveredId,
			$elm$core$Maybe$Just(pieceId));
		var dragAttrs = isLocked ? _List_Nil : _List_fromArray(
			[
				A2($elm$html$Html$Attributes$attribute, 'draggable', 'true'),
				A2(
				$elm$html$Html$Events$on,
				'dragstart',
				$elm$json$Json$Decode$succeed(
					$author$project$Main$DragPieceStart(pieceId))),
				A2(
				$elm$html$Html$Events$on,
				'dragend',
				$elm$json$Json$Decode$succeed($author$project$Main$DragPieceEnd)),
				A2(
				$elm$html$Html$Events$stopPropagationOn,
				'dragenter',
				$elm$json$Json$Decode$succeed(
					_Utils_Tuple2(
						$author$project$Main$DragEnterPiece(pieceId),
						true)))
			]);
		return A2(
			$elm$html$Html$div,
			_Utils_ap(
				_List_fromArray(
					[
						$elm$html$Html$Attributes$classList(
						_List_fromArray(
							[
								_Utils_Tuple2('piece-thumb', true),
								_Utils_Tuple2('hovered', isHovered)
							])),
						$elm$html$Html$Events$onMouseEnter(
						$author$project$Main$SetHoveredPiece(
							$elm$core$Maybe$Just(pieceId))),
						$elm$html$Html$Events$onMouseLeave(
						$author$project$Main$SetHoveredPiece($elm$core$Maybe$Nothing))
					]),
				dragAttrs),
			_Utils_ap(
				_List_fromArray(
					[
						A2(
						$elm$html$Html$img,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$src(dataUrl),
								A2($elm$html$Html$Attributes$style, 'max-height', '48px'),
								A2($elm$html$Html$Attributes$style, 'max-width', '80px'),
								A2($elm$html$Html$Attributes$style, 'display', 'block')
							]),
						_List_Nil)
					]),
				_Utils_ap(
					function () {
						if (!maybePos.$) {
							var pos = maybePos.a;
							return _List_fromArray(
								[
									A2(
									$elm$html$Html$div,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('tray-thumb-num')
										]),
									_List_fromArray(
										[
											$elm$html$Html$text(
											$elm$core$String$fromInt(pos))
										]))
								]);
						} else {
							return _List_Nil;
						}
					}(),
					function () {
						if (!removeInfo.$) {
							var _v2 = removeInfo.a;
							var wid = _v2.a;
							var pid = _v2.b;
							return _List_fromArray(
								[
									A2(
									$elm$html$Html$button,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('piece-thumb-remove'),
											$elm$html$Html$Events$onClick(
											A2($author$project$Main$RemovePieceFromWave, wid, pid)),
											$elm$html$Html$Attributes$disabled(isLocked),
											$elm$html$Html$Attributes$title('Remove from wave')
										]),
									_List_fromArray(
										[
											$elm$html$Html$text('\u2715')
										]))
								]);
						} else {
							return _List_Nil;
						}
					}())));
	});
var $author$project$Main$viewGroupRow = F3(
	function (model, allGroups, group) {
		var swatchColor = A2($author$project$Main$waveColor, group.r, 0.85);
		var isSelected = _Utils_eq(
			model.C,
			$elm$core$Maybe$Just(group.a));
		var groupCount = $elm$core$List$length(allGroups);
		return A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$classList(
					_List_fromArray(
						[
							_Utils_Tuple2('wave-row', true),
							_Utils_Tuple2('selected', isSelected),
							_Utils_Tuple2(
							'drag-over',
							_Utils_eq(
								model.ae,
								$elm$core$Maybe$Just(
									$elm$core$Maybe$Just(group.a))))
						])),
					A2(
					$elm$html$Html$Events$preventDefaultOn,
					'dragover',
					$elm$json$Json$Decode$succeed(
						_Utils_Tuple2($author$project$Main$NoOp, true))),
					A2(
					$elm$html$Html$Events$on,
					'dragenter',
					$elm$json$Json$Decode$succeed(
						$author$project$Main$DragEnterGroup(
							$elm$core$Maybe$Just(group.a)))),
					A2(
					$elm$html$Html$Events$on,
					'drop',
					$elm$json$Json$Decode$succeed(
						$author$project$Main$DropOnGroup(
							$elm$core$Maybe$Just(group.a))))
				]),
			_List_fromArray(
				[
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('wave-row-header'),
							$elm$html$Html$Events$onClick(
							(isSelected && (groupCount > 1)) ? $author$project$Main$SelectGroup($elm$core$Maybe$Nothing) : $author$project$Main$SelectGroup(
								$elm$core$Maybe$Just(group.a)))
						]),
					_List_fromArray(
						[
							A2(
							$elm$html$Html$span,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$classList(
									_List_fromArray(
										[
											_Utils_Tuple2('wave-lock', true),
											_Utils_Tuple2('locked', group.g)
										])),
									A2(
									$elm$html$Html$Events$stopPropagationOn,
									'click',
									$elm$json$Json$Decode$succeed(
										_Utils_Tuple2(
											$author$project$Main$ToggleGroupLock(group.a),
											true))),
									$elm$html$Html$Attributes$title(
									group.g ? 'Unlock group' : 'Lock group')
								]),
							_List_fromArray(
								[
									group.g ? $author$project$Main$iconLockClosed : $author$project$Main$iconLockOpen
								])),
							A2(
							$elm$html$Html$span,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('wave-swatch'),
									A2($elm$html$Html$Attributes$style, 'background-color', swatchColor),
									A2(
									$elm$html$Html$Events$stopPropagationOn,
									'mousedown',
									A3(
										$elm$json$Json$Decode$map2,
										F2(
											function (mx, my) {
												return _Utils_Tuple2(
													A3(
														$author$project$Main$StartColorPick,
														$author$project$Main$GroupColorTarget(group.a),
														mx,
														my),
													true);
											}),
										A2($elm$json$Json$Decode$field, 'clientX', $elm$json$Json$Decode$float),
										A2($elm$json$Json$Decode$field, 'clientY', $elm$json$Json$Decode$float))),
									$elm$html$Html$Attributes$title('Pick color')
								]),
							_List_Nil),
							A2(
							$elm$html$Html$span,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('wave-piece-count-label'),
									A2(
									$elm$html$Html$Attributes$style,
									'color',
									A2($author$project$Main$waveColor, group.r, 1.0))
								]),
							_List_fromArray(
								[
									$elm$html$Html$text(
									$elm$core$String$fromInt(
										$elm$core$List$length(group.b)) + ' pcs')
								])),
							A2(
							$elm$html$Html$span,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('wave-name-label')
								]),
							_List_fromArray(
								[
									$elm$html$Html$text(group.N)
								])),
							A2(
							$elm$html$Html$span,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('wave-row-spacer')
								]),
							_List_Nil),
							A2(
							$elm$html$Html$span,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('wave-actions')
								]),
							_List_fromArray(
								[
									A2(
									$elm$html$Html$button,
									_List_fromArray(
										[
											A2(
											$elm$html$Html$Events$stopPropagationOn,
											'click',
											$elm$json$Json$Decode$succeed(
												_Utils_Tuple2(
													$author$project$Main$RemoveGroup(group.a),
													true))),
											$elm$html$Html$Attributes$disabled(groupCount <= 1),
											$elm$html$Html$Attributes$title('Delete group')
										]),
									_List_fromArray(
										[
											$elm$html$Html$text('\u2715')
										]))
								]))
						])),
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('wave-pieces')
						]),
					A2(
						$elm$core$List$filterMap,
						function (pid) {
							return A2(
								$elm$core$Maybe$map,
								function (piece) {
									return A6(
										$author$project$Main$viewPieceThumb,
										$elm$core$Maybe$Just(
											_Utils_Tuple2(group.a, pid)),
										false,
										model.M,
										pid,
										piece.G + ('?v=' + $elm$core$String$fromInt(model.s)),
										$elm$core$Maybe$Nothing);
								},
								$elm$core$List$head(
									A2(
										$elm$core$List$filter,
										function (p) {
											return _Utils_eq(p.a, pid);
										},
										model.d)));
						},
						group.b))
				]));
	});
var $author$project$Main$viewGroupUnassignedRow = F2(
	function (model, unassignedPieces) {
		return $elm$core$List$isEmpty(model.d) ? $elm$html$Html$text('') : A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$classList(
					_List_fromArray(
						[
							_Utils_Tuple2('wave-row', true),
							_Utils_Tuple2(
							'drag-over',
							_Utils_eq(
								model.ae,
								$elm$core$Maybe$Just($elm$core$Maybe$Nothing)))
						])),
					A2(
					$elm$html$Html$Events$preventDefaultOn,
					'dragover',
					$elm$json$Json$Decode$succeed(
						_Utils_Tuple2($author$project$Main$NoOp, true))),
					A2(
					$elm$html$Html$Events$on,
					'dragenter',
					$elm$json$Json$Decode$succeed(
						$author$project$Main$DragEnterGroup($elm$core$Maybe$Nothing))),
					A2(
					$elm$html$Html$Events$on,
					'drop',
					$elm$json$Json$Decode$succeed(
						$author$project$Main$DropOnGroup($elm$core$Maybe$Nothing)))
				]),
			_List_fromArray(
				[
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('wave-row-header')
						]),
					_List_fromArray(
						[
							A2(
							$elm$html$Html$span,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('wave-label unassigned-label')
								]),
							_List_fromArray(
								[
									$elm$html$Html$text('Unassigned')
								])),
							A2(
							$elm$html$Html$span,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('wave-piece-count')
								]),
							_List_fromArray(
								[
									$elm$html$Html$text(
									$elm$core$String$fromInt(
										$elm$core$List$length(unassignedPieces)) + ' pcs')
								]))
						])),
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('wave-pieces')
						]),
					A2(
						$elm$core$List$map,
						function (p) {
							return A6(
								$author$project$Main$viewPieceThumb,
								$elm$core$Maybe$Nothing,
								false,
								model.M,
								p.a,
								p.G + ('?v=' + $elm$core$String$fromInt(model.s)),
								$elm$core$Maybe$Nothing);
						},
						unassignedPieces))
				]));
	});
var $author$project$Main$viewGroupsTools = function (model) {
	var totalPieces = $elm$core$List$length(model.d);
	var assignedIds = A2(
		$elm$core$List$concatMap,
		function ($) {
			return $.b;
		},
		model.f);
	var unassignedPieces = A2(
		$elm$core$List$filter,
		function (p) {
			return !A2($elm$core$List$member, p.a, assignedIds);
		},
		model.d);
	var assignedCount = $elm$core$List$length(assignedIds);
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('tools-pane waves-tools')
			]),
		_List_fromArray(
			[
				$author$project$Main$viewTogglesBox(
				_List_fromArray(
					[
						$author$project$Main$viewCheckboxLights(model),
						$author$project$Main$viewCheckboxGrid(model),
						$author$project$Main$viewCheckboxOutlines(model),
						$author$project$Main$viewCheckboxGroupOverlay(model)
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('waves-header')
					]),
				_List_fromArray(
					[
						$author$project$Main$viewSectionTitle('Groups'),
						A2(
						$elm$html$Html$span,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('wave-count')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text(
								(totalPieces > 0) ? ($elm$core$String$fromInt(assignedCount) + ('/' + $elm$core$String$fromInt(totalPieces))) : '')
							]))
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('wave-toolbar')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$button,
						_List_fromArray(
							[
								$elm$html$Html$Events$onClick($author$project$Main$AddGroup)
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('New group')
							]))
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('waves-body')
					]),
				_Utils_ap(
					A2(
						$elm$core$List$map,
						A2($author$project$Main$viewGroupRow, model, model.f),
						model.f),
					_List_fromArray(
						[
							A2($author$project$Main$viewGroupUnassignedRow, model, unassignedPieces)
						])))
			]));
};
var $author$project$Main$StartEdit = {$: 30};
var $elm$core$Maybe$andThen = F2(
	function (callback, maybeValue) {
		if (!maybeValue.$) {
			var value = maybeValue.a;
			return callback(value);
		} else {
			return $elm$core$Maybe$Nothing;
		}
	});
var $author$project$Main$CancelEdit = {$: 33};
var $author$project$Main$SaveEdit = {$: 32};
var $elm$html$Html$br = _VirtualDom_node('br');
var $elm$core$List$sort = function (xs) {
	return A2($elm$core$List$sortBy, $elm$core$Basics$identity, xs);
};
var $author$project$Main$editHasChanges = function (model) {
	return !_Utils_eq(
		$elm$core$List$sort(model.n),
		$elm$core$List$sort(model.J));
};
var $author$project$Main$viewEditControls = function (model) {
	var pieceLabel = function () {
		var _v0 = model.A;
		if (!_v0.$) {
			var pid = _v0.a;
			return 'Piece #' + pid;
		} else {
			return 'Piece';
		}
	}();
	var changed = $author$project$Main$editHasChanges(model);
	var brickCount = $elm$core$List$length(model.n);
	return _List_fromArray(
		[
			A2(
			$elm$html$Html$h2,
			_List_Nil,
			_List_fromArray(
				[
					$elm$html$Html$text('Editing ' + pieceLabel)
				])),
			A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					A2($elm$html$Html$Attributes$style, 'font-size', '11px'),
					A2($elm$html$Html$Attributes$style, 'color', '#aaa'),
					A2($elm$html$Html$Attributes$style, 'margin-bottom', '10px'),
					A2($elm$html$Html$Attributes$style, 'line-height', '1.5')
				]),
			_List_fromArray(
				[
					$elm$html$Html$text('Click bricks to add/remove.'),
					A2($elm$html$Html$br, _List_Nil, _List_Nil),
					$elm$html$Html$text(
					$elm$core$String$fromInt(brickCount) + (' brick' + (((brickCount === 1) ? '' : 's') + ' selected.')))
				])),
			A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$class('btn-row')
				]),
			_List_fromArray(
				[
					A2(
					$elm$html$Html$button,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('primary'),
							$elm$html$Html$Events$onClick($author$project$Main$SaveEdit),
							$elm$html$Html$Attributes$disabled(!changed)
						]),
					_List_fromArray(
						[
							$elm$html$Html$text('Save')
						])),
					A2(
					$elm$html$Html$button,
					_List_fromArray(
						[
							$elm$html$Html$Events$onClick($author$project$Main$CancelEdit)
						]),
					_List_fromArray(
						[
							$elm$html$Html$text('Cancel')
						]))
				]))
		]);
};
var $author$project$Main$viewPiecesTools = function (model) {
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('tools-pane')
			]),
		function () {
			if (model.m) {
				return $author$project$Main$viewEditControls(model);
			} else {
				var selectedPiece = A2(
					$elm$core$Maybe$andThen,
					function (pid) {
						return $elm$core$List$head(
							A2(
								$elm$core$List$filter,
								function (p) {
									return _Utils_eq(p.a, pid);
								},
								model.d));
					},
					model.A);
				return _List_fromArray(
					[
						$author$project$Main$viewTogglesBox(
						_List_fromArray(
							[
								$author$project$Main$viewCheckboxLights(model),
								$author$project$Main$viewCheckboxGrid(model),
								$author$project$Main$viewCheckboxOutlines(model)
							])),
						$author$project$Main$viewSectionTitle('Edit Pieces'),
						function () {
						if (!selectedPiece.$) {
							var piece = selectedPiece.a;
							return A2(
								$elm$html$Html$div,
								_List_fromArray(
									[
										$elm$html$Html$Attributes$class('piece-info')
									]),
								_List_fromArray(
									[
										A2(
										$elm$html$Html$div,
										_List_fromArray(
											[
												$elm$html$Html$Attributes$class('piece-info-row')
											]),
										_List_fromArray(
											[
												$elm$html$Html$text('Piece ID: ' + piece.a)
											])),
										A2(
										$elm$html$Html$div,
										_List_fromArray(
											[
												$elm$html$Html$Attributes$class('piece-info-row')
											]),
										_List_fromArray(
											[
												$elm$html$Html$text(
												'Bricks: ' + $elm$core$String$fromInt(
													$elm$core$List$length(piece.u)))
											])),
										A2(
										$elm$html$Html$div,
										_List_fromArray(
											[
												$elm$html$Html$Attributes$class('piece-info-row')
											]),
										_List_fromArray(
											[
												$elm$html$Html$text(
												'Brick IDs: ' + A2($elm$core$String$join, ', ', piece.u))
											])),
										A2(
										$elm$html$Html$button,
										_List_fromArray(
											[
												$elm$html$Html$Attributes$class('primary'),
												$elm$html$Html$Events$onClick($author$project$Main$StartEdit),
												$elm$html$Html$Attributes$disabled(model.x)
											]),
										_List_fromArray(
											[
												$elm$html$Html$text('Edit Piece')
											]))
									]));
						} else {
							return A2(
								$elm$html$Html$div,
								_List_fromArray(
									[
										$elm$html$Html$Attributes$class('piece-info-empty')
									]),
								_List_fromArray(
									[
										$elm$html$Html$text('Click a piece to select')
									]));
						}
					}()
					]);
			}
		}());
};
var $author$project$Main$AddWave = {$: 21};
var $author$project$Main$DragEnterWave = function (a) {
	return {$: 45, a: a};
};
var $author$project$Main$DropOnWave = function (a) {
	return {$: 47, a: a};
};
var $author$project$Main$viewGroupThumb = F8(
	function (maybeWaveId, hoveredId, maybeGroup, piece, allIds, generation, maybePos, isLocked) {
		var n = $elm$core$List$length(allIds);
		var isHovered = _Utils_eq(
			hoveredId,
			$elm$core$Maybe$Just(piece.a));
		var dragAttrs = isLocked ? _List_Nil : _List_fromArray(
			[
				A2($elm$html$Html$Attributes$attribute, 'draggable', 'true'),
				A2(
				$elm$html$Html$Events$on,
				'dragstart',
				$elm$json$Json$Decode$succeed(
					$author$project$Main$DragPieceStart(piece.a))),
				A2(
				$elm$html$Html$Events$on,
				'dragend',
				$elm$json$Json$Decode$succeed($author$project$Main$DragPieceEnd)),
				A2(
				$elm$html$Html$Events$stopPropagationOn,
				'dragenter',
				$elm$json$Json$Decode$succeed(
					_Utils_Tuple2(
						$author$project$Main$DragEnterPiece(piece.a),
						true)))
			]);
		var clickMsg = function () {
			var _v1 = _Utils_Tuple2(maybeGroup, maybeWaveId);
			if ((!_v1.a.$) && (!_v1.b.$)) {
				var g = _v1.a.a;
				var wid = _v1.b.a;
				return A2($author$project$Main$AssignGroupToWave, g.a, wid);
			} else {
				return $author$project$Main$NoOp;
			}
		}();
		return A2(
			$elm$html$Html$div,
			_Utils_ap(
				_List_fromArray(
					[
						$elm$html$Html$Attributes$classList(
						_List_fromArray(
							[
								_Utils_Tuple2('piece-thumb', true),
								_Utils_Tuple2('hovered', isHovered)
							])),
						$elm$html$Html$Events$onMouseEnter(
						$author$project$Main$SetHoveredPiece(
							$elm$core$Maybe$Just(piece.a))),
						$elm$html$Html$Events$onMouseLeave(
						$author$project$Main$SetHoveredPiece($elm$core$Maybe$Nothing)),
						$elm$html$Html$Events$onClick(clickMsg)
					]),
				dragAttrs),
			_List_fromArray(
				[
					A2(
					$elm$html$Html$img,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$src(
							piece.G + ('?v=' + $elm$core$String$fromInt(generation))),
							A2($elm$html$Html$Attributes$style, 'max-height', '48px'),
							A2($elm$html$Html$Attributes$style, 'max-width', '80px'),
							A2($elm$html$Html$Attributes$style, 'display', 'block')
						]),
					_List_Nil),
					function () {
					if (!maybePos.$) {
						var pos = maybePos.a;
						return A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('piece-thumb-pos')
								]),
							_List_fromArray(
								[
									$elm$html$Html$text(
									$elm$core$String$fromInt(pos))
								]));
					} else {
						return $elm$html$Html$text('');
					}
				}(),
					(n > 1) ? A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('group-xn-badge group-xn-badge-bottom')
						]),
					_List_fromArray(
						[
							$elm$html$Html$text(
							'x' + $elm$core$String$fromInt(n))
						])) : $elm$html$Html$text('')
				]));
	});
var $author$project$Main$viewUnassignedRow = F2(
	function (model, unassignedPieces) {
		return $elm$core$List$isEmpty(model.d) ? $elm$html$Html$text('') : A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$classList(
					_List_fromArray(
						[
							_Utils_Tuple2('wave-row', true),
							_Utils_Tuple2(
							'drag-over',
							_Utils_eq(
								model.R,
								$elm$core$Maybe$Just($elm$core$Maybe$Nothing)))
						])),
					A2(
					$elm$html$Html$Events$preventDefaultOn,
					'dragover',
					$elm$json$Json$Decode$succeed(
						_Utils_Tuple2($author$project$Main$NoOp, true))),
					A2(
					$elm$html$Html$Events$on,
					'dragenter',
					$elm$json$Json$Decode$succeed(
						$author$project$Main$DragEnterWave($elm$core$Maybe$Nothing))),
					A2(
					$elm$html$Html$Events$on,
					'drop',
					$elm$json$Json$Decode$succeed(
						$author$project$Main$DropOnWave($elm$core$Maybe$Nothing)))
				]),
			_List_fromArray(
				[
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('wave-row-header')
						]),
					_List_fromArray(
						[
							A2(
							$elm$html$Html$span,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('wave-label unassigned-label')
								]),
							_List_fromArray(
								[
									$elm$html$Html$text('Unassigned')
								])),
							A2(
							$elm$html$Html$span,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('wave-piece-count')
								]),
							_List_fromArray(
								[
									$elm$html$Html$text(
									$elm$core$String$fromInt(
										$elm$core$List$length(unassignedPieces)) + ' pcs')
								]))
						])),
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('wave-pieces')
						]),
					A2(
						$elm$core$List$filterMap,
						function (display) {
							if (!display.$) {
								var pid = display.a;
								return A2(
									$elm$core$Maybe$map,
									function (p) {
										return A6(
											$author$project$Main$viewPieceThumb,
											$elm$core$Maybe$Nothing,
											false,
											model.M,
											p.a,
											p.G + ('?v=' + $elm$core$String$fromInt(model.s)),
											$elm$core$Maybe$Nothing);
									},
									$elm$core$List$head(
										A2(
											$elm$core$List$filter,
											function (p) {
												return _Utils_eq(p.a, pid);
											},
											model.d)));
							} else {
								var repId = display.a;
								var allIds = display.b;
								return A2(
									$elm$core$Maybe$map,
									function (p) {
										return A8(
											$author$project$Main$viewGroupThumb,
											model.k,
											model.M,
											$elm$core$List$head(
												A2(
													$elm$core$List$filter,
													function (g) {
														return A2($elm$core$List$member, repId, g.b);
													},
													model.f)),
											p,
											allIds,
											model.s,
											$elm$core$Maybe$Nothing,
											false);
									},
									$elm$core$List$head(
										A2(
											$elm$core$List$filter,
											function (p) {
												return _Utils_eq(p.a, repId);
											},
											model.d)));
							}
						},
						A2(
							$author$project$Main$toPieceDisplays,
							model.f,
							A2(
								$elm$core$List$map,
								function ($) {
									return $.a;
								},
								unassignedPieces))))
				]));
	});
var $author$project$Main$viewWavePieceInfoBox = function (model) {
	var waveOfPiece = function (pid) {
		return A2(
			$elm$core$Maybe$map,
			$elm$core$Tuple$first,
			$elm$core$List$head(
				A2(
					$elm$core$List$filter,
					function (_v5) {
						var wv = _v5.b;
						return A2($elm$core$List$member, pid, wv.b);
					},
					A2(
						$elm$core$List$indexedMap,
						F2(
							function (i, wv) {
								return _Utils_Tuple2(i + 1, wv);
							}),
						model.c))));
	};
	var piecePositions = $elm$core$Dict$fromList(
		A2(
			$elm$core$List$concatMap,
			function (wv) {
				return A2(
					$elm$core$List$indexedMap,
					F2(
						function (i, pid) {
							return _Utils_Tuple2(pid, i + 1);
						}),
					wv.b);
			},
			model.c));
	var focusId = function () {
		var _v4 = model.M;
		if (!_v4.$) {
			var pid = _v4.a;
			return $elm$core$Maybe$Just(pid);
		} else {
			return model.A;
		}
	}();
	if (!focusId.$) {
		var pid = focusId.a;
		var posLabel = function () {
			var _v2 = A2($elm$core$Dict$get, pid, piecePositions);
			if (!_v2.$) {
				var pos = _v2.a;
				var _v3 = waveOfPiece(pid);
				if (!_v3.$) {
					var waveNum = _v3.a;
					return 'Wave ' + ($elm$core$String$fromInt(waveNum) + (', pos ' + $elm$core$String$fromInt(pos)));
				} else {
					return 'pos ' + $elm$core$String$fromInt(pos);
				}
			} else {
				return 'Unassigned';
			}
		}();
		var maybePiece = $elm$core$List$head(
			A2(
				$elm$core$List$filter,
				function (p) {
					return _Utils_eq(p.a, pid);
				},
				model.d));
		return A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$class('stats')
				]),
			function () {
				if (!maybePiece.$) {
					var piece = maybePiece.a;
					return _List_fromArray(
						[
							A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('row')
								]),
							_List_fromArray(
								[
									A2(
									$elm$html$Html$span,
									_List_Nil,
									_List_fromArray(
										[
											$elm$html$Html$text('Position')
										])),
									A2(
									$elm$html$Html$span,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('val')
										]),
									_List_fromArray(
										[
											$elm$html$Html$text(posLabel)
										]))
								])),
							A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('row')
								]),
							_List_fromArray(
								[
									A2(
									$elm$html$Html$span,
									_List_Nil,
									_List_fromArray(
										[
											$elm$html$Html$text('Piece ID')
										])),
									A2(
									$elm$html$Html$span,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('val')
										]),
									_List_fromArray(
										[
											$elm$html$Html$text(pid)
										]))
								])),
							A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('row')
								]),
							_List_fromArray(
								[
									A2(
									$elm$html$Html$span,
									_List_Nil,
									_List_fromArray(
										[
											$elm$html$Html$text('Bricks')
										])),
									A2(
									$elm$html$Html$span,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('val')
										]),
									_List_fromArray(
										[
											$elm$html$Html$text(
											$elm$core$String$fromInt(
												$elm$core$List$length(piece.u)))
										]))
								])),
							A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('row')
								]),
							_List_fromArray(
								[
									A2(
									$elm$html$Html$span,
									_List_Nil,
									_List_fromArray(
										[
											$elm$html$Html$text('Brick IDs')
										])),
									A2(
									$elm$html$Html$span,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('val')
										]),
									_List_fromArray(
										[
											$elm$html$Html$text(
											A2($elm$core$String$join, ', ', piece.u))
										]))
								]))
						]);
				} else {
					return _List_fromArray(
						[
							A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('row')
								]),
							_List_fromArray(
								[
									A2(
									$elm$html$Html$span,
									_List_Nil,
									_List_fromArray(
										[
											$elm$html$Html$text('Position')
										])),
									A2(
									$elm$html$Html$span,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('val')
										]),
									_List_fromArray(
										[
											$elm$html$Html$text(posLabel)
										]))
								])),
							A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('row')
								]),
							_List_fromArray(
								[
									A2(
									$elm$html$Html$span,
									_List_Nil,
									_List_fromArray(
										[
											$elm$html$Html$text('Piece ID')
										])),
									A2(
									$elm$html$Html$span,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('val')
										]),
									_List_fromArray(
										[
											$elm$html$Html$text(pid)
										]))
								]))
						]);
				}
			}());
	} else {
		return A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$class('stats')
				]),
			_List_fromArray(
				[
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('row')
						]),
					_List_fromArray(
						[
							A2(
							$elm$html$Html$span,
							_List_fromArray(
								[
									A2($elm$html$Html$Attributes$style, 'color', '#aaa'),
									A2($elm$html$Html$Attributes$style, 'font-style', 'italic')
								]),
							_List_fromArray(
								[
									$elm$html$Html$text('Hover a piece to inspect')
								]))
						]))
				]));
	}
};
var $author$project$Main$RemoveWave = function (a) {
	return {$: 29, a: a};
};
var $author$project$Main$SelectWave = function (a) {
	return {$: 25, a: a};
};
var $author$project$Main$ToggleWaveLock = function (a) {
	return {$: 48, a: a};
};
var $author$project$Main$ToggleWaveVisibility = function (a) {
	return {$: 22, a: a};
};
var $author$project$Main$WaveColorTarget = function (a) {
	return {$: 0, a: a};
};
var $author$project$Main$iconEye = A2(
	$elm$svg$Svg$svg,
	_List_fromArray(
		[
			$elm$svg$Svg$Attributes$viewBox('0 0 24 24'),
			$elm$svg$Svg$Attributes$width('14'),
			$elm$svg$Svg$Attributes$height('14'),
			$elm$svg$Svg$Attributes$fill('currentColor')
		]),
	_List_fromArray(
		[
			A2(
			$elm$svg$Svg$path,
			_List_fromArray(
				[
					$elm$svg$Svg$Attributes$d('M23.271,9.419C21.72,6.893,18.192,2.655,12,2.655S2.28,6.893.729,9.419a4.908,4.908,0,0,0,0,5.162C2.28,17.107,5.808,21.345,12,21.345s9.72-4.238,11.271-6.764A4.908,4.908,0,0,0,23.271,9.419Zm-1.705,4.115C20.234,15.7,17.219,19.345,12,19.345S3.766,15.7,2.434,13.534a2.918,2.918,0,0,1,0-3.068C3.766,8.3,6.781,4.655,12,4.655s8.234,3.641,9.566,5.811A2.918,2.918,0,0,1,21.566,13.534Z')
				]),
			_List_Nil),
			A2(
			$elm$svg$Svg$path,
			_List_fromArray(
				[
					$elm$svg$Svg$Attributes$d('M12,7a5,5,0,1,0,5,5A5.006,5.006,0,0,0,12,7Zm0,8a3,3,0,1,1,3-3A3,3,0,0,1,12,15Z')
				]),
			_List_Nil)
		]));
var $author$project$Main$iconEyeCrossed = A2(
	$elm$svg$Svg$svg,
	_List_fromArray(
		[
			$elm$svg$Svg$Attributes$viewBox('0 0 24 24'),
			$elm$svg$Svg$Attributes$width('14'),
			$elm$svg$Svg$Attributes$height('14'),
			$elm$svg$Svg$Attributes$fill('currentColor')
		]),
	_List_fromArray(
		[
			A2(
			$elm$svg$Svg$path,
			_List_fromArray(
				[
					$elm$svg$Svg$Attributes$d('M23.271,9.419A15.866,15.866,0,0,0,19.9,5.51l2.8-2.8a1,1,0,0,0-1.414-1.414L18.241,4.345A12.054,12.054,0,0,0,12,2.655C5.809,2.655,2.281,6.893.729,9.419a4.908,4.908,0,0,0,0,5.162A15.866,15.866,0,0,0,4.1,18.49l-2.8,2.8a1,1,0,1,0,1.414,1.414l3.052-3.052A12.054,12.054,0,0,0,12,21.345c6.191,0,9.719-4.238,11.271-6.764A4.908,4.908,0,0,0,23.271,9.419ZM2.433,13.534a2.918,2.918,0,0,1,0-3.068C3.767,8.3,6.782,4.655,12,4.655A10.1,10.1,0,0,1,16.766,5.82L14.753,7.833a4.992,4.992,0,0,0-6.92,6.92l-2.31,2.31A13.723,13.723,0,0,1,2.433,13.534ZM15,12a3,3,0,0,1-3,3,2.951,2.951,0,0,1-1.285-.3L14.7,10.715A2.951,2.951,0,0,1,15,12ZM9,12a3,3,0,0,1,3-3,2.951,2.951,0,0,1,1.285.3L9.3,13.285A2.951,2.951,0,0,1,9,12Zm12.567,1.534C20.233,15.7,17.218,19.345,12,19.345A10.1,10.1,0,0,1,7.234,18.18l2.013-2.013a4.992,4.992,0,0,0,6.92-6.92l2.31-2.31a13.723,13.723,0,0,1,3.09,3.529A2.918,2.918,0,0,1,21.567,13.534Z')
				]),
			_List_Nil)
		]));
var $author$project$Main$viewWaveRow = F3(
	function (model, allWaves, wave) {
		var waveCount = $elm$core$List$length(allWaves);
		var swatchColor = A2($author$project$Main$waveColor, wave.r, 0.85);
		var isSelected = _Utils_eq(
			model.k,
			$elm$core$Maybe$Just(wave.a));
		var countColor = A2($author$project$Main$waveColor, wave.r, 1.0);
		return A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$classList(
					_List_fromArray(
						[
							_Utils_Tuple2('wave-row', true),
							_Utils_Tuple2('selected', isSelected),
							_Utils_Tuple2('locked', wave.g),
							_Utils_Tuple2(
							'drag-over',
							(!wave.g) && _Utils_eq(
								model.R,
								$elm$core$Maybe$Just(
									$elm$core$Maybe$Just(wave.a))))
						])),
					A2(
					$elm$html$Html$Events$preventDefaultOn,
					'dragover',
					$elm$json$Json$Decode$succeed(
						_Utils_Tuple2($author$project$Main$NoOp, true))),
					A2(
					$elm$html$Html$Events$on,
					'dragenter',
					$elm$json$Json$Decode$succeed(
						$author$project$Main$DragEnterWave(
							$elm$core$Maybe$Just(wave.a)))),
					A2(
					$elm$html$Html$Events$on,
					'drop',
					$elm$json$Json$Decode$succeed(
						$author$project$Main$DropOnWave(
							$elm$core$Maybe$Just(wave.a))))
				]),
			_List_fromArray(
				[
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('wave-row-header'),
							$elm$html$Html$Events$onClick(
							(isSelected && (waveCount > 1)) ? $author$project$Main$SelectWave($elm$core$Maybe$Nothing) : $author$project$Main$SelectWave(
								$elm$core$Maybe$Just(wave.a)))
						]),
					_List_fromArray(
						[
							A2(
							$elm$html$Html$span,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$classList(
									_List_fromArray(
										[
											_Utils_Tuple2('wave-eye', true),
											_Utils_Tuple2('hidden', !wave.V)
										])),
									A2(
									$elm$html$Html$Events$stopPropagationOn,
									'click',
									$elm$json$Json$Decode$succeed(
										_Utils_Tuple2(
											$author$project$Main$ToggleWaveVisibility(wave.a),
											true))),
									$elm$html$Html$Attributes$title(
									wave.V ? 'Hide wave' : 'Show wave')
								]),
							_List_fromArray(
								[
									wave.V ? $author$project$Main$iconEye : $author$project$Main$iconEyeCrossed
								])),
							A2(
							$elm$html$Html$span,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$classList(
									_List_fromArray(
										[
											_Utils_Tuple2('wave-lock', true),
											_Utils_Tuple2('locked', wave.g)
										])),
									A2(
									$elm$html$Html$Events$stopPropagationOn,
									'click',
									$elm$json$Json$Decode$succeed(
										_Utils_Tuple2(
											$author$project$Main$ToggleWaveLock(wave.a),
											true))),
									$elm$html$Html$Attributes$title(
									wave.g ? 'Unlock wave' : 'Lock wave')
								]),
							_List_fromArray(
								[
									wave.g ? $author$project$Main$iconLockClosed : $author$project$Main$iconLockOpen
								])),
							A2(
							$elm$html$Html$span,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('wave-swatch'),
									A2($elm$html$Html$Attributes$style, 'background-color', swatchColor),
									A2(
									$elm$html$Html$Events$stopPropagationOn,
									'mousedown',
									A3(
										$elm$json$Json$Decode$map2,
										F2(
											function (mx, my) {
												return _Utils_Tuple2(
													A3(
														$author$project$Main$StartColorPick,
														$author$project$Main$WaveColorTarget(wave.a),
														mx,
														my),
													true);
											}),
										A2($elm$json$Json$Decode$field, 'clientX', $elm$json$Json$Decode$float),
										A2($elm$json$Json$Decode$field, 'clientY', $elm$json$Json$Decode$float))),
									$elm$html$Html$Attributes$title('Pick color')
								]),
							_List_Nil),
							A2(
							$elm$html$Html$span,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('wave-piece-count-label'),
									A2($elm$html$Html$Attributes$style, 'color', countColor)
								]),
							_List_fromArray(
								[
									$elm$html$Html$text(
									$elm$core$String$fromInt(
										$elm$core$List$length(wave.b)) + ' pcs')
								])),
							A2(
							$elm$html$Html$span,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('wave-name-label')
								]),
							_List_fromArray(
								[
									$elm$html$Html$text(wave.N)
								])),
							A2(
							$elm$html$Html$span,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('wave-row-spacer')
								]),
							_List_Nil),
							A2(
							$elm$html$Html$span,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('wave-actions')
								]),
							_List_fromArray(
								[
									A2(
									$elm$html$Html$button,
									_List_fromArray(
										[
											A2(
											$elm$html$Html$Events$stopPropagationOn,
											'click',
											$elm$json$Json$Decode$succeed(
												_Utils_Tuple2(
													$author$project$Main$RemoveWave(wave.a),
													true))),
											$elm$html$Html$Attributes$disabled(waveCount <= 1),
											$elm$html$Html$Attributes$title('Delete wave')
										]),
									_List_fromArray(
										[
											$elm$html$Html$text('\u2715')
										]))
								]))
						])),
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('wave-pieces')
						]),
					A2(
						$elm$core$List$filterMap,
						function (_v0) {
							var pos = _v0.a;
							var display = _v0.b;
							if (!display.$) {
								var pid = display.a;
								return A2(
									$elm$core$Maybe$map,
									function (piece) {
										return A6(
											$author$project$Main$viewPieceThumb,
											$elm$core$Maybe$Just(
												_Utils_Tuple2(wave.a, pid)),
											wave.g,
											model.M,
											pid,
											piece.G + ('?v=' + $elm$core$String$fromInt(model.s)),
											$elm$core$Maybe$Just(pos));
									},
									$elm$core$List$head(
										A2(
											$elm$core$List$filter,
											function (p) {
												return _Utils_eq(p.a, pid);
											},
											model.d)));
							} else {
								var repId = display.a;
								var allIds = display.b;
								return A2(
									$elm$core$Maybe$map,
									function (piece) {
										return A8(
											$author$project$Main$viewGroupThumb,
											$elm$core$Maybe$Just(wave.a),
											model.M,
											$elm$core$List$head(
												A2(
													$elm$core$List$filter,
													function (g) {
														return A2($elm$core$List$member, repId, g.b);
													},
													model.f)),
											piece,
											allIds,
											model.s,
											$elm$core$Maybe$Just(pos),
											wave.g);
									},
									$elm$core$List$head(
										A2(
											$elm$core$List$filter,
											function (p) {
												return _Utils_eq(p.a, repId);
											},
											model.d)));
							}
						},
						A2(
							$elm$core$List$indexedMap,
							F2(
								function (i, display) {
									return _Utils_Tuple2(i + 1, display);
								}),
							A2($author$project$Main$toPieceDisplays, model.f, wave.b))))
				]));
	});
var $author$project$Main$viewWavesTools = function (model) {
	var totalPieces = $elm$core$List$length(model.d);
	var assignedIds = A2(
		$elm$core$List$concatMap,
		function ($) {
			return $.b;
		},
		model.c);
	var unassignedPieces = A2(
		$elm$core$List$filter,
		function (p) {
			return !A2($elm$core$List$member, p.a, assignedIds);
		},
		model.d);
	var assignedCount = $elm$core$List$length(assignedIds);
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('tools-pane waves-tools')
			]),
		_List_fromArray(
			[
				$author$project$Main$viewTogglesBox(
				_List_fromArray(
					[
						$author$project$Main$viewCheckboxLights(model),
						$author$project$Main$viewCheckboxGrid(model),
						$author$project$Main$viewCheckboxOutlines(model),
						$author$project$Main$viewCheckboxWaveOverlay(model),
						$author$project$Main$viewCheckboxNumbers(model)
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('waves-header')
					]),
				_List_fromArray(
					[
						$author$project$Main$viewSectionTitle('Waves'),
						A2(
						$elm$html$Html$span,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('wave-count')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text(
								(totalPieces > 0) ? ($elm$core$String$fromInt(assignedCount) + ('/' + $elm$core$String$fromInt(totalPieces))) : '')
							]))
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('wave-toolbar')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$button,
						_List_fromArray(
							[
								$elm$html$Html$Events$onClick($author$project$Main$AddWave)
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('New wave')
							]))
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('waves-body')
					]),
				_Utils_ap(
					A2(
						$elm$core$List$map,
						A2($author$project$Main$viewWaveRow, model, model.c),
						model.c),
					_List_fromArray(
						[
							A2($author$project$Main$viewUnassignedRow, model, unassignedPieces)
						]))),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('tools-divider')
					]),
				_List_Nil),
				$author$project$Main$viewWavePieceInfoBox(model)
			]));
};
var $author$project$Main$viewToolsCol = F2(
	function (model, response) {
		return A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$class('tools-col')
				]),
			_List_fromArray(
				[
					function () {
					var _v0 = model.e;
					switch (_v0) {
						case 0:
							return $elm$html$Html$text('');
						case 1:
							return A2($author$project$Main$viewGenerateTools, model, response);
						case 2:
							return $author$project$Main$viewPiecesTools(model);
						case 3:
							return $author$project$Main$viewBlueprintTools(model);
						case 4:
							return $author$project$Main$viewGroupsTools(model);
						case 5:
							return $author$project$Main$viewWavesTools(model);
						default:
							return $author$project$Main$viewExportTools(model);
					}
				}()
				]));
	});
var $author$project$Main$viewBody = function (model) {
	if (!model.e) {
		return A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$class('app-body-empty')
				]),
			_List_fromArray(
				[
					$author$project$Main$viewFileList(model),
					$author$project$Main$viewBodyOverlay(model)
				]));
	} else {
		var _v0 = model.o;
		if (_v0.$ === 2) {
			var response = _v0.a;
			return A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('app-body')
					]),
				_List_fromArray(
					[
						A2($author$project$Main$viewCanvasCol, model, response),
						A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('resize-handle')
							]),
						_List_Nil),
						A2($author$project$Main$viewToolsCol, model, response),
						$author$project$Main$viewBodyOverlay(model)
					]));
		} else {
			return A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('app-body')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('canvas-col')
							]),
						_List_fromArray(
							[
								A2(
								$elm$html$Html$div,
								_List_fromArray(
									[
										$elm$html$Html$Attributes$class('canvas-area')
									]),
								_List_fromArray(
									[
										A2(
										$elm$html$Html$div,
										_List_fromArray(
											[
												$elm$html$Html$Attributes$class('canvas-spinner-overlay')
											]),
										_List_fromArray(
											[
												A2(
												$elm$html$Html$div,
												_List_fromArray(
													[
														$elm$html$Html$Attributes$class('canvas-spinner')
													]),
												_List_Nil)
											]))
									]))
							])),
						A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('tools-col')
							]),
						_List_Nil)
					]));
		}
	}
};
var $author$project$Main$ScrollTrayBy = function (a) {
	return {$: 67, a: a};
};
var $author$project$Main$viewWaveTrayThumb = F8(
	function (piece, isLocked, scale, hoveredId, generation, showNum, pos, maybeGroupN) {
		var widthCss = $elm$core$String$fromFloat(piece.l * scale) + 'px';
		var isHovered = _Utils_eq(
			hoveredId,
			$elm$core$Maybe$Just(piece.a));
		var dragAttrs = isLocked ? _List_Nil : _List_fromArray(
			[
				A2($elm$html$Html$Attributes$attribute, 'draggable', 'true'),
				A2(
				$elm$html$Html$Events$on,
				'dragstart',
				$elm$json$Json$Decode$succeed(
					$author$project$Main$DragPieceStart(piece.a))),
				A2(
				$elm$html$Html$Events$on,
				'dragend',
				$elm$json$Json$Decode$succeed($author$project$Main$DragPieceEnd)),
				A2(
				$elm$html$Html$Events$stopPropagationOn,
				'dragenter',
				$elm$json$Json$Decode$succeed(
					_Utils_Tuple2(
						$author$project$Main$DragEnterPiece(piece.a),
						true)))
			]);
		return A2(
			$elm$html$Html$div,
			_Utils_ap(
				_List_fromArray(
					[
						$elm$html$Html$Attributes$classList(
						_List_fromArray(
							[
								_Utils_Tuple2('wave-tray-thumb', true),
								_Utils_Tuple2('hovered', isHovered)
							])),
						A2($elm$html$Html$Attributes$style, 'width', widthCss),
						A2(
						$elm$html$Html$Attributes$style,
						'aspect-ratio',
						$elm$core$String$fromFloat(piece.l / piece.j)),
						$elm$html$Html$Events$onMouseEnter(
						$author$project$Main$SetHoveredPiece(
							$elm$core$Maybe$Just(piece.a))),
						$elm$html$Html$Events$onMouseLeave(
						$author$project$Main$SetHoveredPiece($elm$core$Maybe$Nothing))
					]),
				dragAttrs),
			_List_fromArray(
				[
					A2(
					$elm$html$Html$img,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$src(
							piece.G + ('?v=' + $elm$core$String$fromInt(generation)))
						]),
					_List_Nil),
					showNum ? A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('tray-thumb-num')
						]),
					_List_fromArray(
						[
							$elm$html$Html$text(
							$elm$core$String$fromInt(pos))
						])) : $elm$html$Html$text(''),
					function () {
					if (!maybeGroupN.$) {
						var n = maybeGroupN.a;
						return A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('group-xn-badge group-xn-badge-bottom')
								]),
							_List_fromArray(
								[
									$elm$html$Html$text(
									'x' + $elm$core$String$fromInt(n))
								]));
					} else {
						return $elm$html$Html$text('');
					}
				}()
				]));
	});
var $author$project$Main$viewWaveTray = F2(
	function (model, _v0) {
		var activeWaveId = model.k;
		var activeWave = $elm$core$List$head(
			A2(
				$elm$core$List$filter,
				function (w) {
					return _Utils_eq(
						$elm$core$Maybe$Just(w.a),
						activeWaveId);
				},
				model.c));
		var activeWavePieceIds = A2(
			$elm$core$Maybe$withDefault,
			_List_Nil,
			A2(
				$elm$core$Maybe$map,
				function ($) {
					return $.b;
				},
				activeWave));
		var isLocked = A2(
			$elm$core$Maybe$withDefault,
			false,
			A2(
				$elm$core$Maybe$map,
				function ($) {
					return $.g;
				},
				activeWave));
		return A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$classList(
					_List_fromArray(
						[
							_Utils_Tuple2('wave-tray', true),
							_Utils_Tuple2(
							'drag-over',
							(!isLocked) && _Utils_eq(
								model.R,
								$elm$core$Maybe$Just(activeWaveId)))
						])),
					A2(
					$elm$html$Html$Events$preventDefaultOn,
					'dragover',
					$elm$json$Json$Decode$succeed(
						_Utils_Tuple2($author$project$Main$NoOp, true))),
					A2(
					$elm$html$Html$Events$on,
					'dragenter',
					$elm$json$Json$Decode$succeed(
						$author$project$Main$DragEnterWave(activeWaveId))),
					A2(
					$elm$html$Html$Events$on,
					'drop',
					$elm$json$Json$Decode$succeed(
						$author$project$Main$DropOnWave(activeWaveId)))
				]),
			_List_fromArray(
				[
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('wave-tray-bg'),
							A2(
							$elm$html$Html$Events$preventDefaultOn,
							'wheel',
							A3(
								$elm$json$Json$Decode$map2,
								F2(
									function (dx, dy) {
										return _Utils_Tuple2(
											$author$project$Main$ScrollTrayBy(
												(!(!dx)) ? dx : dy),
											true);
									}),
								A2($elm$json$Json$Decode$field, 'deltaX', $elm$json$Json$Decode$float),
								A2($elm$json$Json$Decode$field, 'deltaY', $elm$json$Json$Decode$float)))
						]),
					_List_Nil),
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('wave-tray-scroll'),
							$elm$html$Html$Attributes$id('wave-tray-scroll')
						]),
					function () {
						var endMarker = ((!isLocked) && ((!_Utils_eq(model.F, $elm$core$Maybe$Nothing)) && (_Utils_eq(model.Q, $elm$core$Maybe$Nothing) && _Utils_eq(
							model.R,
							$elm$core$Maybe$Just(activeWaveId))))) ? _List_fromArray(
							[
								A2(
								$elm$html$Html$div,
								_List_fromArray(
									[
										$elm$html$Html$Attributes$class('drag-insert-marker-v')
									]),
								_List_Nil)
							]) : _List_Nil;
						var displays = A2(
							$elm$core$List$indexedMap,
							F2(
								function (i, display) {
									return _Utils_Tuple2(i + 1, display);
								}),
							A2($author$project$Main$toPieceDisplays, model.f, activeWavePieceIds));
						var thumbs = A2(
							$elm$core$List$concatMap,
							function (_v1) {
								var pos = _v1.a;
								var display = _v1.b;
								var repId = function () {
									if (!display.$) {
										var pid = display.a;
										return pid;
									} else {
										var pid = display.a;
										return pid;
									}
								}();
								var showMarker = (!isLocked) && ((!_Utils_eq(model.F, $elm$core$Maybe$Nothing)) && _Utils_eq(
									model.Q,
									$elm$core$Maybe$Just(repId)));
								var thumb = function () {
									var _v2 = $elm$core$List$head(
										A2(
											$elm$core$List$filter,
											function (p) {
												return _Utils_eq(p.a, repId);
											},
											model.d));
									if (!_v2.$) {
										var piece = _v2.a;
										var groupCount = function () {
											if (!display.$) {
												return $elm$core$Maybe$Nothing;
											} else {
												var allIds = display.b;
												return $elm$core$Maybe$Just(
													$elm$core$List$length(allIds));
											}
										}();
										return _List_fromArray(
											[
												A8($author$project$Main$viewWaveTrayThumb, piece, isLocked, model.aR, model.M, model.s, model.aw, pos, groupCount)
											]);
									} else {
										return _List_Nil;
									}
								}();
								var marker = showMarker ? _List_fromArray(
									[
										A2(
										$elm$html$Html$div,
										_List_fromArray(
											[
												$elm$html$Html$Attributes$class('drag-insert-marker-v')
											]),
										_List_Nil)
									]) : _List_Nil;
								return _Utils_ap(marker, thumb);
							},
							displays);
						return _Utils_ap(thumbs, endMarker);
					}())
				]));
	});
var $author$project$Main$viewBottomWaveTray = function (model) {
	if (model.e !== 5) {
		return _List_Nil;
	} else {
		var _v0 = model.o;
		if (_v0.$ === 2) {
			var response = _v0.a;
			return _List_fromArray(
				[
					A2($author$project$Main$viewWaveTray, model, response)
				]);
		} else {
			return _List_Nil;
		}
	}
};
var $author$project$Main$viewColorPickerPanel = function (model) {
	var _v0 = model.O;
	if (_v0.$ === 1) {
		return $elm$html$Html$text('');
	} else {
		var cp = _v0.a;
		return A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$class('color-picker-panel'),
					A2(
					$elm$html$Html$Attributes$style,
					'left',
					$elm$core$String$fromFloat(cp.a5) + 'px'),
					A2(
					$elm$html$Html$Attributes$style,
					'top',
					$elm$core$String$fromFloat(cp.a6) + 'px')
				]),
			_List_fromArray(
				[
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('color-picker-row')
						]),
					_List_fromArray(
						[
							A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('color-picker-bw')
								]),
							_List_fromArray(
								[
									A2(
									$elm$html$Html$div,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('bw-swatch bw-black'),
											$elm$html$Html$Attributes$title('Black')
										]),
									_List_Nil),
									A2(
									$elm$html$Html$div,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('bw-swatch bw-white'),
											$elm$html$Html$Attributes$title('White')
										]),
									_List_Nil)
								])),
							A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class(
									cp.a1 ? 'color-picker-inner hue-only' : 'color-picker-inner')
								]),
							_List_fromArray(
								[
									A2(
									$elm$html$Html$div,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('color-picker-gradient')
										]),
									_List_Nil)
								]))
						]))
				]));
	}
};
var $author$project$Main$Reset = {$: 5};
var $author$project$Main$SetAppMode = function (a) {
	return {$: 13, a: a};
};
var $author$project$Main$viewTitleBar = function (model) {
	var isLoadingPdf = _Utils_eq(model.o, $author$project$Main$Loading);
	var isLoaded = function () {
		var _v0 = model.o;
		if (_v0.$ === 2) {
			return true;
		} else {
			return false;
		}
	}();
	var isGenerating = model.y === 1;
	var isGenerated = model.y === 2;
	var isBusy = isLoadingPdf || (model.x || model.S);
	var hasFile = !$elm$core$String$isEmpty(model.av);
	var assignedIds = A2(
		$elm$core$List$concatMap,
		function ($) {
			return $.b;
		},
		model.c);
	var hasUnassigned = A2(
		$elm$core$List$any,
		function (p) {
			return !A2($elm$core$List$member, p.a, assignedIds);
		},
		model.d);
	var canExport = isGenerated && ((!isBusy) && ((!isGenerating) && (!hasUnassigned)));
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('left-sidebar')
			]),
		_List_fromArray(
			[
				A2(
				$elm$html$Html$span,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('app-title')
					]),
				_List_fromArray(
					[
						$elm$html$Html$text('House Puzzle')
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('sidebar-nav')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$button,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$classList(
								_List_fromArray(
									[
										_Utils_Tuple2('mode-btn', true),
										_Utils_Tuple2('active', !model.e),
										_Utils_Tuple2('loading', isLoadingPdf)
									])),
								$elm$html$Html$Attributes$disabled(isBusy || isGenerating),
								$elm$html$Html$Events$onClick($author$project$Main$Reset)
							]),
						_List_fromArray(
							[
								$elm$html$Html$text(
								isLoadingPdf ? 'Loading\u2026' : (hasFile ? 'Reset' : 'Start'))
							])),
						A2(
						$elm$html$Html$span,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('mode-sep')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('\u2193')
							])),
						A2(
						$elm$html$Html$button,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$classList(
								_List_fromArray(
									[
										_Utils_Tuple2('mode-btn', true),
										_Utils_Tuple2('active', model.e === 1),
										_Utils_Tuple2('loading', isGenerating)
									])),
								$elm$html$Html$Attributes$disabled((!isLoaded) || (isBusy || isGenerating)),
								$elm$html$Html$Events$onClick(
								$author$project$Main$SetAppMode(1))
							]),
						_List_fromArray(
							[
								$elm$html$Html$text(
								isGenerating ? 'Importing\u2026' : 'Import')
							])),
						A2(
						$elm$html$Html$span,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('mode-sep')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('\u2193')
							])),
						A2(
						$elm$html$Html$button,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$classList(
								_List_fromArray(
									[
										_Utils_Tuple2('mode-btn', true),
										_Utils_Tuple2('active', model.e === 2),
										_Utils_Tuple2('loading', model.x && (model.e === 2))
									])),
								$elm$html$Html$Attributes$disabled((!isGenerated) || (isBusy || isGenerating)),
								$elm$html$Html$Events$onClick(
								$author$project$Main$SetAppMode(2))
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Pieces')
							])),
						A2(
						$elm$html$Html$span,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('mode-sep')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('\u2195')
							])),
						A2(
						$elm$html$Html$button,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$classList(
								_List_fromArray(
									[
										_Utils_Tuple2('mode-btn', true),
										_Utils_Tuple2('active', model.e === 3)
									])),
								$elm$html$Html$Attributes$disabled((!isGenerated) || (isBusy || isGenerating)),
								$elm$html$Html$Events$onClick(
								$author$project$Main$SetAppMode(3))
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Blueprint')
							])),
						A2(
						$elm$html$Html$span,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('mode-sep')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('\u2195')
							])),
						A2(
						$elm$html$Html$button,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$classList(
								_List_fromArray(
									[
										_Utils_Tuple2('mode-btn', true),
										_Utils_Tuple2('active', model.e === 4)
									])),
								$elm$html$Html$Attributes$disabled((!isGenerated) || (isBusy || isGenerating)),
								$elm$html$Html$Events$onClick(
								$author$project$Main$SetAppMode(4))
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Groups')
							])),
						A2(
						$elm$html$Html$span,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('mode-sep')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('\u2193')
							])),
						A2(
						$elm$html$Html$button,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$classList(
								_List_fromArray(
									[
										_Utils_Tuple2('mode-btn', true),
										_Utils_Tuple2('active', model.e === 5)
									])),
								$elm$html$Html$Attributes$disabled((!isGenerated) || (isBusy || isGenerating)),
								$elm$html$Html$Events$onClick(
								$author$project$Main$SetAppMode(5))
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Waves')
							])),
						A2(
						$elm$html$Html$span,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('mode-sep')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('\u2193')
							])),
						A2(
						$elm$html$Html$button,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$classList(
								_List_fromArray(
									[
										_Utils_Tuple2('mode-btn', true),
										_Utils_Tuple2('export-btn', true),
										_Utils_Tuple2('active', model.e === 6),
										_Utils_Tuple2('loading', model.S)
									])),
								$elm$html$Html$Attributes$disabled(!canExport),
								$elm$html$Html$Events$onClick(
								$author$project$Main$SetAppMode(6)),
								$elm$html$Html$Attributes$title(
								(hasUnassigned && isGenerated) ? 'All pieces must be assigned to waves before exporting' : '')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Export')
							]))
					])),
				A2(
				$elm$html$Html$span,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('version-tag')
					]),
				_List_fromArray(
					[
						$elm$html$Html$text(model.a_)
					]))
			]));
};
var $author$project$Main$view = function (model) {
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('app')
			]),
		_Utils_ap(
			_List_fromArray(
				[
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('app-main')
						]),
					_List_fromArray(
						[
							$author$project$Main$viewTitleBar(model),
							$author$project$Main$viewBody(model),
							$author$project$Main$viewColorPickerPanel(model)
						]))
				]),
			$author$project$Main$viewBottomWaveTray(model)));
};
var $author$project$Main$main = $elm$browser$Browser$element(
	{bU: $author$project$Main$init, b3: $author$project$Main$subscriptions, b5: $author$project$Main$update, b6: $author$project$Main$view});
_Platform_export({'Main':{'init':$author$project$Main$main(
	A2(
		$elm$json$Json$Decode$andThen,
		function (version) {
			return $elm$json$Json$Decode$succeed(
				{bH: version});
		},
		A2($elm$json$Json$Decode$field, 'version', $elm$json$Json$Decode$string)))(0)}});}(this));