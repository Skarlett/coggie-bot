
fn setup_client() {
    let regex = Regex::new(r"^(http|https?:\/\/(?:www\.|(?!www))[^\s\.]+\.[^\s]{2,3}|www\.[^\s]+\.[^\s]{2,3})$").unwrap();

    regex.compile()
}


// we have to use regex because discord now supports
// link masking as markdown syntax
async fn on_message(ctx: &Context, msg: &Message) {   
    let regex = ctx.data.read().await.get::<UrlValidate>().unwrap();    

    let captures = regex.captures(&msg.content);

    for url in captures {
        // wait for #128 to be merged

        println!("url: {}", url);
    }    
}