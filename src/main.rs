use cargo_nbuild::{App, AppOptions, Result};

fn main() -> Result<()> {
  let opt = AppOptions::default().parse();
  App::new(opt).run()
}
