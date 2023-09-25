
enum Lexer {
    Int,
    Colon,
}


struct PythonIndices {
    start: isize,
    end: isize,
    skip: isize
}


//  1 -> (0) hot-queue
//  1: -> (1..) queued only 
//  -1 -> end ( -1) back only 
// [INT]

//  :: -> (0..) ALL
//  [COLON][IMP][COLON][IMP]

//  3::-1 -> (0, 1, 2)
//  3::   -> (3, 4, 5, ...)
//  1:6:2 -> (1, 3, 5)
//  :5     -> (..5) queued + front only
impl PythonIndices {
    fn from_str(payload: &str) {
        let mut field_ident = 0;
        let mut iter = payload.split(":");
        let mut use_last = true;
        let mut last = 1;

        let first = match iter.next() {
            Some("") => 0,
            Some(x) => x.parse::<isize>()
                .unwrap_or(0),
            None => return
        };        

        let end = match iter.next() {
            Some("") => isize::MAX,
            Some(x) => {
                let x = x.parse::<isize>().unwrap_or(0);
                if first >= 0 && 0 > x {
                    // probab some heklerino
                    return
                }

                x
            }

            // make sure qmod doesn't run
            None => {

                let handle = qctx.cold_queue.write().await;
                let keep = qctx.cold_queue.write().await.split_off(first);
                
                if first > 0 {
                    handle.clear();
                    handle.extend(keep);
                }

                else {
                    handle.split_off(handle.len() - first.abs() as usize);
                }

                   
            }
        };
        
        let qmod = iter.next();
        
        let skip = match qmod {
            // probob clever heklerino
            Some("0") => return,
            Some("") => 1,
            Some(x) => x.parse::<isize>().unwrap_or(1),                
            

            // process first + last
            None => {
                let reverse = 0 > first && 0 > end; 
                let positive = |s,e| s > e;
                let negative = |s,e| s < e; 
                
                let handle = qctx.cold_queue.write().await;
                let keep = qctx.cold_queue.write().await.split_off(first);
                
                invalid_check = if reverse {
                    positive
                } else { 
                    negative
                };

                if invalid_check(start, end) {
                    return;
                }
           }

                        
            todo!()
        };

        let reverse = 0 > skip;
        let skip = skip.abs();

        let mut cold_queue = qctx.cold_queue.write().await;
        
        cold_queue.split_off(first as usize);



        if reverse {
        }

        for x in cold_queue.step_by(skip as usize) {
            x.clone();
        }        


        match reverse {
            true => cold_queue.pop_back(),
        }       
         

    }
}