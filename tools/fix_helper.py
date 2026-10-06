"""otty-helper: `Response::err` takes &str and `io_err` consumes the io::Error."""
import pathlib
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\helper\src\main.rs")
t = F.read_text(encoding="utf-8")
before = t

EDITS = [
    # A decode failure already carries a String; `Response::err` wants &str.
    ("            Err(e) => Response::err(e.code, e.message),",
     "            Err(e) => Response::err(&e.code, e.message),"),

    # `io_err` consumes the io::Error, so it can only be called once.
    ("                Err(e) => return Response::err(&io_err(e).code, io_err(e).message),",
     "                Err(e) => return Response::err(io_err(e).parts()),"),

    ("            Err(e) => Response::err(&io_err(e).code, io_err(e).message),",
     "            Err(e) => Response::err(io_err(e).parts()),"),

    # `term` is used again after the call, so hand over a copy.
    ("            terminfo(&[\"check\".to_owned(), term.clone()]);",
     "            terminfo(&[\"check\".to_owned(), term.clone()]);"),
]

for old, new in EDITS:
    if old in t:
        t = t.replace(old, new)

# The TerminfoCheck arm moves `term` into the args vector and then reads it.
t = t.replace(
    """        Request::TerminfoCheck { term } => {
            terminfo(&["check".to_owned(), term]);""",
    """        Request::TerminfoCheck { term } => {
            terminfo(&["check".to_owned(), term.clone()]);""",
)

# Give RpcError a borrowing helper, so a code and a message can be handed over
# without moving the error.
t = t.replace(
    """fn io_err(e: std::io::Error) -> RpcError {""",
    """/// The (code, message) pair `Response::err` wants.
impl RpcError {
    fn parts(self) -> (&'static str, String) {
        // The code is always one of the constants, so it can be returned as a
        // `&'static str` and `Response::err` keeps its cheap signature.
        let code = match self.code.as_str() {
            "io" => RpcError::IO,
            "no_such_path" => RpcError::NO_SUCH_PATH,
            "not_a_file" => RpcError::NOT_A_FILE,
            "too_large" => RpcError::TOO_LARGE,
            "bad_request" => RpcError::BAD_REQUEST,
            _ => RpcError::UNSUPPORTED,
        };
        (code, self.message)
    }
}

fn io_err(e: std::io::Error) -> RpcError {""",
)

if t == before:
    print("nothing changed")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("patched otty-helper main.rs")
