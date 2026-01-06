<!-- TOC -->
* [LANGULO](#langulo)
  * [A SMALL TOUR](#a-small-tour)
    * [EXPRESSIONS](#expressions)
    * [FUNCTIONS](#functions)
    * [NULLABILITY AND IF/ELSE](#nullability-and-ifelse)
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
```

[...]

TODO

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