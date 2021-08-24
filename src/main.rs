use hacker_news::client::html_client::Client;

fn main() {
    let client = Client::new("", ""); 
    let listings = client.news();
    println!("listing = {:#?}", listings);
}
