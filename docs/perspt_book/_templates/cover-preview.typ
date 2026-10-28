#import "book.typ": book
#show: book.with(
  title: "Perspt: AI chat and coding in the terminal",
  author: "Vikrant Rathore",
  version: sys.inputs.at("version", default: "0.6.6"),
  logo: "../../shared/_static/perspt-logo.svg",
)
