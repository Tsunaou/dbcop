use std::collections::{HashMap, HashSet};

#[derive(Debug)]
struct EdgeClosure {
    forward_edge: HashMap<usize, HashSet<usize>>,
    backward_edge: HashMap<usize, HashSet<usize>>,
}

impl EdgeClosure {
    pub fn new() -> Self {
        EdgeClosure {
            forward_edge: HashMap::new(),
            backward_edge: HashMap::new(),
        }
    }

    pub fn contains(&self, u: usize, v: usize) -> bool {
        self.forward_edge
            .get(&u)
            .and_then(|vs| Some(vs.contains(&v))) == Some(true)
    }

    pub fn add_edge(&mut self, u: usize, v: usize) -> bool {
        if !self.contains(u, v) {
            let mut new_edge = Vec::new();
            {
                let opt_prevs_u = self.backward_edge.get(&u);
                let opt_nexts_v = self.forward_edge.get(&v);
                if let Some(prevs_u) = opt_prevs_u {
                    if let Some(nexts_v) = opt_nexts_v {
                        for &prev_u in prevs_u.iter() {
                            for &next_v in nexts_v.iter() {
                                if !self.contains(prev_u, next_v) {
                                    new_edge.push((prev_u, next_v));
                                }
                            }
                        }
                    }
                }
                if let Some(prevs_u) = opt_prevs_u {
                    for &prev_u in prevs_u.iter() {
                        if !self.contains(prev_u, v) {
                            new_edge.push((prev_u, v));
                        }
                    }
                }
                if let Some(nexts_v) = opt_nexts_v {
                    for &next_v in nexts_v.iter() {
                        if !self.contains(u, next_v) {
                            new_edge.push((u, next_v));
                        }
                    }
                }
                new_edge.push((u, v));
            }
            for (u_, v_) in new_edge {
                let ent_u = self.forward_edge.entry(u_).or_insert_with(HashSet::new);
                ent_u.insert(v_);
                let ent_v = self.backward_edge.entry(v_).or_insert_with(HashSet::new);
                ent_v.insert(u_);
            }
            true
        } else {
            false
        }
    }
}

#[derive(Debug)]
pub struct Chains {
    n_sizes: Vec<usize>,
    root_txn_id: usize,
    txns: Vec<Option<(HashMap<usize, usize>, HashSet<usize>)>>,

    tuple_to_id: Vec<Vec<usize>>,
    id_to_tuple: Vec<(usize, usize)>,
    wr_order: HashMap<usize, HashMap<usize, HashSet<usize>>>,
    wr_order_by_txn: HashMap<usize, HashMap<usize, HashSet<usize>>>,
    vis_closure: EdgeClosure,
}

impl Chains {
    pub fn new(
        n_sizes: &Vec<usize>,
        txns_info: &HashMap<(usize, usize), (HashMap<usize, (usize, usize)>, HashSet<usize>)>,
    ) -> Self {
        let root_txn_id = 0;
        let mut id_to_tuple = Vec::with_capacity(n_sizes.iter().sum::<usize>() + 1usize);
        let mut tuple_to_id = vec![Vec::new(); n_sizes.len() + 1];

        tuple_to_id[root_txn_id].push(id_to_tuple.len());
        id_to_tuple.push((root_txn_id, 0));

        for (node_id, &node_len) in n_sizes.iter().enumerate() {
            let curr_po = &mut tuple_to_id[node_id + 1];
            for node_ix in 0..node_len {
                curr_po.push(id_to_tuple.len());
                id_to_tuple.push((node_id + 1, node_ix));
            }
        }
        let mut txns = vec![None; n_sizes.iter().sum::<usize>() + 1];
        let mut root_writes = HashSet::new();

        for (&(node_id1, txn_id1), (_rd_info, _wr_info)) in txns_info.iter() {
            let mut rd_info = HashMap::new();
            let mut wr_info = HashSet::new();
            for (&x, (node_id2, txn_id2)) in _rd_info.iter() {
                rd_info.insert(x, tuple_to_id[*node_id2][*txn_id2]);
                if tuple_to_id[*node_id2][*txn_id2] == root_txn_id {
                    root_writes.insert(x);
                }
            }
            for &x in _wr_info.iter() {
                wr_info.insert(x);
            }

            txns[tuple_to_id[node_id1][txn_id1]] = Some((rd_info, wr_info));
        }

        txns[root_txn_id] = Some((HashMap::new(), root_writes));

        Chains {
            n_sizes: n_sizes.clone(),
            root_txn_id: root_txn_id,
            txns: txns,
            wr_order: HashMap::new(),
            wr_order_by_txn: HashMap::new(),
            vis_closure: EdgeClosure::new(),
            id_to_tuple: id_to_tuple,
            tuple_to_id: tuple_to_id,
        }
    }

    pub fn preprocess_wr(&mut self) {
        for (txn, s) in self.txns.iter().enumerate() {
            if let Some((rd_info, wr_info)) = s {
                for (&x, &wr_txn) in rd_info {
                    {
                        let var_ent = self.wr_order.entry(x).or_insert_with(HashMap::new);
                        let txn_ent = var_ent.entry(wr_txn).or_insert_with(HashSet::new);
                        txn_ent.insert(txn);
                    }
                    {
                        let txn_ent = self.wr_order_by_txn
                            .entry(wr_txn)
                            .or_insert_with(HashMap::new);
                        let var_ent = txn_ent.entry(x).or_insert_with(HashSet::new);
                        var_ent.insert(txn);
                    }
                }

                for &x in wr_info.iter() {
                    {
                        let var_ent = self.wr_order.entry(x).or_insert_with(HashMap::new);
                        var_ent.entry(txn).or_insert_with(HashSet::new);
                    }
                    {
                        let txn_ent = self.wr_order_by_txn.entry(txn).or_insert_with(HashMap::new);
                        txn_ent.entry(x).or_insert_with(HashSet::new);
                    }
                }
            }
        }

        // says, 0 writes all vars at beginning
        // is this necessary?
        // for (_, hm) in self.wr_order.iter_mut() {
        //     hm.entry(self.root_txn_id).or_insert_with(HashSet::new);
        // }
        //
        // {
        //     let root_ent = self.wr_order_by_txn
        //         .entry(self.root_txn_id)
        //         .or_insert_with(HashMap::new);
        //     for (&x, _) in self.wr_order.iter_mut() {
        //         root_ent.entry(x).or_insert_with(HashSet::new);
        //     }
        // }
    }

    pub fn preprocess_vis(&mut self) {
        for po in self.tuple_to_id.iter().skip(1) {
            for (j, &id) in po.iter().enumerate() {
                if j < po.len() - 1 {
                    self.vis_closure.add_edge(id, id + 1);
                }
            }
            if let Some(&u) = po.first() {
                self.vis_closure.add_edge(self.root_txn_id, u);
            }
        }

        for (_, info) in self.wr_order.iter() {
            for (&u, vs) in info {
                for &v in vs.iter() {
                    self.vis_closure.add_edge(u, v);
                }
            }
        }
    }

    pub fn preprocess_ww_rw(&mut self) -> bool {
        loop {
            let mut new_edge = Vec::new();

            for (&_x, wr_x) in self.wr_order.iter() {
                for (&u, vs) in wr_x.iter() {
                    for &v in vs.iter() {
                        for (&u_, _) in wr_x.iter() {
                            if u != u_ && v != u_ {
                                if self.vis_closure.contains(u, u_) {
                                    println!(
                                        "adding RW ({1}, {2}), WR_{3}({0}, {1}), wr({2}) |- {0}, VIS({0}, {2})",
                                        u, v, u_, _x
                                    );
                                    if self.vis_closure.contains(u_, v) {
                                        return false;
                                    }
                                    new_edge.push((v, u_));
                                }
                                if self.vis_closure.contains(u_, v) {
                                    println!(
                                        "adding WW ({2}, {0}), WR_{3}({0}, {1}), wr({2}) |- {0}, VIS({2}, {1})",
                                        u, v, u_, _x
                                    );
                                    if self.vis_closure.contains(u, u_) {
                                        return false;
                                    }
                                    new_edge.push((u_, u));
                                }
                            }
                        }
                    }
                }
            }

            let mut is_converged = true;

            for (u, v) in new_edge {
                if self.vis_closure.contains(v, u) {
                    return false;
                }
                is_converged &= !self.vis_closure.add_edge(u, v);
            }

            if is_converged {
                break;
            }
        }
        return true;
    }

    pub fn preprocess(&mut self) -> bool {
        self.preprocess_wr();
        self.preprocess_vis();
        self.preprocess_ww_rw()
    }

    pub fn _serializable_order_dfs(
        &self,
        cut: &mut Vec<usize>,
        active_prev: &mut HashMap<usize, HashSet<usize>>,
        last_wr: &mut HashMap<usize, (usize, HashSet<usize>)>,
        prev_order: &mut Vec<usize>,
        seen: &mut HashSet<Vec<usize>>,
    ) -> bool {
        if cut[0] == 1
            && cut.iter()
                .skip(1)
                .zip(self.n_sizes.iter())
                .all(|(&l1, &l2)| l1 == l2)
        {
            return true;
        }
        for i in 0..cut.len() {
            cut[i] += 1;
            if cut[i] <= self.tuple_to_id[i].len() && !seen.contains(cut) {
                let cand = self.tuple_to_id[i][cut[i] - 1];
                if !active_prev.contains_key(&cand) {
                    // for _ in 1..cut.iter().sum() {
                    //     print!(" ");
                    // }
                    // println!("{:?}", cut);
                    if let Some((ref rd_info, ref wr_info)) = self.txns[cand] {
                        if wr_info.iter().all(|&x| match last_wr.get(&x) {
                            Some((_, rd_txns)) => rd_txns.iter().all(|&rd_txn| rd_txn == cand),
                            None => true,
                        }) && rd_info.iter().all(|(&x, rf_txn)| {
                            match last_wr.get(&x) {
                                Some((wr_txn, _)) => rf_txn == wr_txn,
                                None => false,
                            }
                        }) {
                            {
                                let mut to_remove = Vec::new();
                                for (x, _) in rd_info.iter() {
                                    let (_, ref mut t) = &mut last_wr.get_mut(x).unwrap();
                                    if t.len() == 1 {
                                        to_remove.push(x);
                                    } else {
                                        if !t.remove(&cand) {
                                            panic!("supposed to be remove some");
                                        }
                                    }
                                }
                                for x in to_remove.iter() {
                                    last_wr.remove(x);
                                }
                            }
                            if let Some(map) = self.wr_order_by_txn.get(&cand) {
                                for (&var, txns) in map.iter() {
                                    last_wr.insert(var, (cand, txns.clone()));
                                }
                            }
                            prev_order.push(cand);
                            if let Some(it) = self.vis_closure.forward_edge.get(&cand) {
                                for &v in it.iter() {
                                    if let Some(s) = active_prev.get_mut(&v) {
                                        s.remove(&cand);
                                    } else {
                                        panic!("this should not raise");
                                    }
                                }
                            }
                            active_prev.retain(|_, v| !v.is_empty());
                            if self._serializable_order_dfs(
                                cut,
                                active_prev,
                                last_wr,
                                prev_order,
                                seen,
                            ) {
                                return true;
                            }
                            // revert last_wr
                            for x in wr_info.iter() {
                                last_wr.remove(x);
                            }
                            for (&x, &rf_txn) in rd_info.iter() {
                                let mut s = HashSet::new();
                                s.insert(cand);
                                last_wr.insert(x, (rf_txn, s));
                            }

                            if let Some(it) = self.vis_closure.forward_edge.get(&cand) {
                                for &v in it.iter() {
                                    let ent = active_prev.entry(v).or_insert_with(HashSet::new);
                                    ent.insert(cand);
                                }
                            }
                            // revert prev_order
                            prev_order.pop();
                            // mark cut as seen
                            seen.insert(cut.clone());
                        }
                    } else {
                        prev_order.push(cand);
                        if let Some(it) = self.vis_closure.forward_edge.get(&cand) {
                            for &v in it.iter() {
                                if let Some(s) = active_prev.get_mut(&v) {
                                    s.remove(&cand);
                                } else {
                                    panic!("this should not raise");
                                }
                            }
                        }
                        active_prev.retain(|_, v| !v.is_empty());
                        if self._serializable_order_dfs(cut, active_prev, last_wr, prev_order, seen)
                        {
                            return true;
                        }
                        if let Some(it) = self.vis_closure.forward_edge.get(&cand) {
                            for &v in it.iter() {
                                let ent = active_prev.entry(v).or_insert_with(HashSet::new);
                                ent.insert(cand);
                            }
                        }
                        // revert prev order
                        prev_order.pop();
                        // mark cut as seen
                        seen.insert(cut.clone());
                    }
                }
            }
            cut[i] -= 1;
        }
        return false;
    }

    pub fn serializable_order_dfs(&self) -> Option<Vec<usize>> {
        // returns a serialization order of each process
        let mut cut = vec![0; self.tuple_to_id.len()];
        let mut active_prev = self.vis_closure.backward_edge.clone();
        let mut last_wr = HashMap::new();
        let mut prev_order = Vec::new();
        let mut seen = HashSet::new();

        if self._serializable_order_dfs(
            &mut cut,
            &mut active_prev,
            &mut last_wr,
            &mut prev_order,
            &mut seen,
        ) {
            Some(
                prev_order
                    .iter()
                    .skip(1)
                    .map(|&id| self.id_to_tuple[id].0)
                    .collect(),
            )
        } else {
            None
        }
    }
}
