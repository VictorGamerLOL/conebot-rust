use anyhow::Result;

/// A paginator for paginating data
#[derive(Debug, Clone)]
pub struct Paginator<T> {
    start: usize,
    end: usize,
    hit_end: bool,
    end_count: usize,
    per_page: usize,
    data: Vec<T>,
}

impl<T> Paginator<T> {
    /// Create a new paginator
    ///
    /// # Arguments
    /// * `data` - The data to paginate
    /// * `per_page` - The number of items per page
    ///
    /// # Returns
    /// A new paginator
    ///
    /// # Errors
    /// - `No data to paginate` if the data is empty
    /// - `Cannot paginate with 0 items per page` if the per_page is 0
    pub fn new(data: Vec<T>, per_page: usize) -> Result<Self> {
        if data.is_empty() {
            return Err(anyhow::anyhow!("No data to paginate"));
        }
        if per_page == 0 {
            return Err(anyhow::anyhow!("Cannot paginate with 0 items per page"));
        }
        let mut end = per_page - 1;
        let mut hit_end = if data.len() < per_page {
            end = data.len() - 1;
            true
        } else {
            false
        };
        Ok(Self {
            start: 0,
            end,
            hit_end: false,
            end_count: data.len() % per_page,
            per_page,
            data,
        })
    }

    /// Get the first page of data
    ///
    /// # Returns
    /// The first page of data, of length `per_page`
    /// or less if there is not enough data.
    pub fn first_page(&mut self) -> &[T] {
        self.start = 0;
        self.hit_end = false;
        self.end = if self.data.len() < self.per_page {
            self.hit_end = true;
            self.data.len() - 1
        } else {
            self.per_page - 1
        };
        &self.data[self.start..=self.end]
    }

    /// Get the next page of data, if it can
    /// continue forward.
    ///
    /// # Returns
    /// The next page of data, of length `per_page`
    /// or less if there is not enough data.
    ///
    /// # Errors
    /// - If the paginator has hit the end, returns `None`
    pub fn next_page(&mut self) -> Option<&[T]> {
        if self.hit_end {
            return None;
        }
        self.start += self.per_page;
        self.end += self.per_page;
        if self.end >= self.data.len() {
            self.end = self.data.len() - 1;
            self.hit_end = true;
        }
        Some(&self.data[self.start..=self.end])
    }

    /// Get the previous page of data, if it can
    /// continue backwards.
    ///
    /// # Returns
    /// The previous page of data, of length `per_page`
    /// or less if there is not enough data.
    ///
    /// # Errors
    /// - If the paginator has hit the start, returns `None`
    pub fn prev_page(&mut self) -> Option<&[T]> {
        if self.start == 0 {
            return None;
        }
        if self.hit_end {
            self.hit_end = false;
            self.end = self.end.saturating_sub(
                if self.end_count == 0 {
                    self.per_page
                } else {
                    self.end_count
                }
            );
            self.start = self.start.saturating_sub(self.per_page);
        } else {
            self.start = self.start.saturating_sub(self.per_page);
            self.end = self.end.saturating_sub(self.per_page);
        }
        Some(&self.data[self.start..=self.end])
    }

    /// Get the current page of data
    ///
    /// # Returns
    /// The current page of data, of length `per_page`
    /// or less if there is not enough data.
    pub fn current_page(&self) -> &[T] {
        &self.data[self.start..=self.end]
    }

    /// Get the last page of data
    ///
    /// # Returns
    /// The last page of data, of length `per_page`
    /// or less if there the total number of items
    /// mod per_page is less than per_page.
    pub fn last_page(&mut self) -> &[T] {
        self.hit_end = true;
        self.end = self.data.len() - 1;
        self.start =
            self.end.saturating_sub(
                if self.end_count == 0 {
                    self.per_page
                } else {
                    self.end_count
                }
            ) + 1; // It's inclusive, if i had 17 and 19, it would return 3 items, not 2. I would need 18 and 19 if I wanted 2 items.
        &self.data[self.start..=self.end]
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_new() {
        let data = vec![1, 2, 3, 4, 5];
        let per_page = 2;
        let paginator = Paginator::new(data, per_page).unwrap();
        assert_eq!(paginator.start, 0);
        assert_eq!(paginator.end, 1);
        assert!(!paginator.hit_end);
        assert_eq!(paginator.end_count, 1);
        assert_eq!(paginator.per_page, 2);
        assert_eq!(paginator.data, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_new_err_no_data() {
        let data: Vec<i32> = vec![];
        let per_page = 2;
        let err = Paginator::new(data, per_page).unwrap_err();
        assert_eq!(err.to_string(), "No data to paginate");
    }

    #[test]
    fn test_new_err_zero_per_page() {
        let data = vec![1, 2, 3, 4, 5];
        let per_page = 0;
        let err = Paginator::new(data, per_page).unwrap_err();
        assert_eq!(err.to_string(), "Cannot paginate with 0 items per page");
    }

    #[test]
    fn test_first_page() {
        let data = vec![1, 2, 3, 4, 5];
        let per_page = 2;
        let mut paginator = Paginator::new(data, per_page).unwrap();
        let page = paginator.first_page();
        assert_eq!(page, vec![1, 2]);
        assert_eq!(paginator.start, 0);
        assert_eq!(paginator.end, 1);
        assert!(!paginator.hit_end);
        assert_eq!(paginator.end_count, 1);
        assert_eq!(paginator.per_page, 2);
        assert_eq!(paginator.data, vec![1, 2, 3, 4, 5]);
    }
    #[test]
    fn test_first_page_less_than_per_page() {
        let data = vec![1, 2];
        let per_page = 3;
        let mut paginator = Paginator::new(data, per_page).unwrap();
        let page = paginator.first_page();
        assert_eq!(page, vec![1, 2]);
        assert_eq!(paginator.start, 0);
        assert_eq!(paginator.end, 1);
        assert!(paginator.hit_end);
        assert_eq!(paginator.end_count, 2);
        assert_eq!(paginator.per_page, 3);
        assert_eq!(paginator.data, vec![1, 2]);
    }

    #[test]
    fn stress_test() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20];
        let per_page = 3;
        let mut paginator = Paginator::new(data, per_page).unwrap();
        assert!(paginator.end_count == 2);
        let page = paginator.first_page();
        assert_eq!(page, vec![1, 2, 3]);
        let page = paginator.next_page().unwrap();
        assert_eq!(page, vec![4, 5, 6]);
        let page = paginator.last_page();
        assert_eq!(page, vec![19, 20]);
        let page = paginator.prev_page().unwrap();
        assert_eq!(page, vec![16, 17, 18]);
        let page = paginator.prev_page().unwrap();
        assert_eq!(page, vec![13, 14, 15]);
        let page = paginator.first_page();
        assert_eq!(page, vec![1, 2, 3]);
        assert!(paginator.prev_page().is_none());
        assert_eq!(paginator.current_page(), vec![1, 2, 3]);
        paginator.next_page();
        paginator.next_page();
        paginator.next_page();
        paginator.next_page();
        paginator.next_page();
        paginator.next_page();
        assert_eq!(paginator.current_page(), vec![19, 20]);
        assert!(paginator.next_page().is_none());
        assert_eq!(paginator.current_page(), vec![19, 20]);
        assert_eq!(paginator.prev_page().unwrap(), vec![16, 17, 18]);
    }
}
