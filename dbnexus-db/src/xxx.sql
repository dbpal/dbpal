WITH aaa(emp_id) as (select * from employees) select * from employees where aaa.emp_id > 0 
72 aaa ; 87 id 