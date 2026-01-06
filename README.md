<!-- TOC -->
* [LANGULO](#langulo)
  * [A SMALL TOUR](#a-small-tour)
    * [EXPRESSIONS](#expressions)
    * [FUNCTIONS](#functions)
    * [NULLABILITY AND IF/ELSE](#nullability-and-ifelse)
    * [MAPS](#maps)
  * [INSTALLATION AND USAGE](#installation-and-usage)
    * [REQUIREMENTS](#requirements)
    * [INSTALL](#install)
    * [USAGE](#usage)
<!-- TOC -->

# LANGULO

Langulo is a toy programming language not meant for serious use. It is:
- written in Rust
- MIT licensed
- transpiled to Python
- dynamically typed
- available as a CLI REPL

## A SMALL TOUR

For a more thorough syntax guide, check the relative [document](docs/SYNTAX.md) (TODO). 

### EXPRESSIONS

Everything[^1] is an expression, meaning that every instruction evaluates to some value.

[^1]: Except for, by necessity, uninstantiated lvalues

In other words, there are no statements.

```
//- 
an example of something that is usually treated as a statement is an assignment.
here, assignments evaluate to the assigned value.
-//

4 + (var = 2) // sets var to 2 and evaluates to 6
```

___

Another typical statement is a `print` instruction. In Langulo, the printing operator `$`
can be attached to *anything* that produces a value (and evaluates to it).

```
subject = "world"
$"hello, {subject}!" // typical print usage.

arithmetic = $4 + 2 $* 3
//-
the multiplication operator produces a value, so it can be printed.
the printing order follows the order of evaluation.
this expression will print 4, then 6, and evaluate to 10.
-//

$new $= $1
//-
printing a lvalue will print its value before the assignment, if any.
otherwise, some fallback text is displayed.
this expression will print (undefined variable `new`), then 1, then 1.
-//

```

### FUNCTIONS

Functions have very light syntax.

```
// declaration
add = |n, m| n + m
// usage
add(2, 3)

// need a bigger function body? use a grouping expression
calculations = | n, m | {
  n = n + 1
  m = m * 2
  n + m
}
//- 
grouping expressions evaluate to either:
- the first return expression they encounter
- the last expression they contain
-//
```

> **NOTE**: for printing purposes, the parentheses in the call are 
> the operand that represents the function call.
> 
> If you wish to print the result of the function:
> `add$(2, 3)`

You can make a function infix by declaring one of its arguments as `@`.

```
// declaration
plus = |@, m| @ + m
// usage
2.plus(3)
// can still be used in its standard form
plus(2, 3)
```

> **NOTE**: in infix form, the operand responsible for the function call becomes `.`.
> The parentheses, while still required syntactically, no longer represent an operation, and thus
> no longer print anything.
> 
> ```
> 2$.plus(3) // prints 5
> 2.plus$(3) // prints nothing
> ```

### NULLABILITY AND IF/ELSE

There is no `null`/`None`. In places where you would usually check for `null`, and as a guardrail against some
types of exceptions, options are used instead. 

> In case you're not familiar with options, they can be thought of as a box, 
> that during the program execution can either be filled with a value or remain empty.

The infix `!` operator boxes/wraps an expression in an option. The value `?` represents an empty option.

```
some_num = 2!
no_num = ?

// unsafe operations can now return an option instead of panicking
somelist = list[1, 2, 3]
somelist[10] // ?
somelist[0] // 1!
```

___

`if` and `else` are here reinterpreted as operators on options.

- `if<condition>: <body>`: if the condition is satisfied, evaluates its body and wraps the result in an option. Otherwise, evaluates to `?`.
- `<option> else <body>`: if `<option>` is filled, evaluates to the wrapped value, otherwise falls back to evaluating the body.

```
hi = 5
lo = 10
// malformed range: evaluates to an empty option.
maybe_range = if hi >= lo: hi - lo
hi = 15
// evaluates to `5!` now that the range is properly defined
maybe_range = if hi >= lo: hi - lo 

somelist = list[1, 2, 3]
somelist[0] else $"empty list" // evaluates to 1, and does not print anything
```
### MAPS

In Langulo, the concept of a collection is very lax. Lists and sets are just convenience constructors
for a generic key/value map, as opposed to more specialized data structures.

```
x = [0: "hi", "hello": 1] // maps can be heterogeneous
list[1, 2] // -> [0: 1, 1: 2]
set[1, 2] // -> [1: true, 2: true]
2..5 // -> [0: 2, 1: 3, 2: 4]
```

___

The main operation you can perform on maps is `iter`. Iter is the only operator that **must** be followed 
by a grouping expression. This is because in this special grouping, Langulo automatically assigns the current
`key`, `value`, and iteration `index` to variables with those names.

```
oldlist = ["three": "four", "one": "two"]
newlist = []
oldlist iter { newlist[index] = "{key}/{value}" } 
//-
this is still a grouping expression, so it evaluates to the last exectued expression.
in this case the value is "three/four". why that and not "one/two"?
because iter always iterates in the sorted order of the map's keys. 
-//

newlist // {0: 'one/two', 1: 'three/four'}


// you can also iter on strings.
"hello" iter { $"char #{index} is '{value}'" } 
//-
char #0 is 'h'
[...]
char #4 is 'o'
-//
```

## INSTALLATION AND USAGE

### REQUIREMENTS

- [cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html)
- [Python 3.7+](https://www.python.org/downloads/)
- optionally, git. Alternatively, you can download the source code as a zip file.

### INSTALL

```bash
git clone https://github.com/efinauri/langulo.git
cd langulo
cargo build --release
```

### USAGE

`cargo run --release`, without additional arguments, starts a REPL session. 

If you wish to evaluate a Langulo program written on a text file, add the relative argument ` -- path/to/file.lgl`.

For example `cargo run --release -- examples/fizzbuzz.lgl`.