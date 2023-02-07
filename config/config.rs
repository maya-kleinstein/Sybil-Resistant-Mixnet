pub fn main(){
    // Get network info through files for mix servers and cliet server

    // Send Mixes start round messages

    // Send client servers generate message requests for servers

    // Send client servers submit messages to mixes 
    // they'll figure out to who themselves ofc, then submit start round when they're done...

    // Mixes should then add messages to everyone, and then get messages from all mixes (as part of start round!)
    // IF next dst is last (aka round % num_layes == 0), then send messages to clients ofc. (different protocol...)
    println!("Please work!");
}