#[derive(clap::Parser)]
pub struct Config {
    #[clap(long, env)]
    pub database_url: String,
    #[clap(long, env)]
    pub aes_key: String,
    #[clap(long, env)]
    pub whatsapp_token: String,
    #[clap(long, env)]
    pub razorpay_key_id: String,
    #[clap(long, env)]
    pub razorpay_key_secret: String,
    #[clap(long, env)]
    pub item_bw: String,
    #[clap(long, env)]
    pub item_bw_t: String,
    #[clap(long, env)]
    pub item_c: String,
    #[clap(long, env)]
    pub item_c_t: String,
    #[clap(long, env)]
    pub webhook_secret: String,
    #[clap(long, env)]
    pub client_secret: String,
    #[clap(long, env)]
    pub port: String,
}
