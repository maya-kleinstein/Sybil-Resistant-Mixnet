use bbs::mix::*;
use bbs::config::*;

#[tokio::test]
async fn system_test(){
    for i in 0..NUM_MIXES {
        tokio::spawn(async move {
            run_mix(i).await.unwrap();
        });
    }
    let result = tokio::spawn(async move {
        return run_config().await.unwrap();
    });
    result.await.unwrap();
}


fn marshal_test(){

}