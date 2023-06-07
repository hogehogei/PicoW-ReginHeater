pub struct Counter
{
    counter: u32,
    max: u32,
}

impl Counter
{
    pub fn new( m: u32 ) -> Self 
    {
        Self {
            counter: 0, max: m
        }
    }

    pub fn count(&mut self, b: bool) -> &Self
    {
        if b {
            if self.counter < self.max {
                self.counter += 1;
            }
        }
        else {
            self.counter = 0;
        }

        self
    }

    pub fn is_reach_limit(&self) -> bool
    {
        self.counter >= self.max
    }

    pub fn reset(&mut self) -> &Self
    {
        self.counter = 0;
        self
    }
}