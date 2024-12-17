TRUNCATE TABLE table_name
26 employees
select id from employees
10 id ; 25 employees
select id from (select id from employees) as xxx
10 id
SELECT e.id AS EmployeeID,  e.name AS EmployeeName,  d.name AS DepartmentName FROM employees AS e JOIN departments AS d ON e.department_id = d.id;
9 e ; 9 employees ; 12 name ; 54 d ; 54 departments ; 60 name ; 93 departments ; 125 e ; 125 departments ; 139 department_id ; 143 d ; 146 id
select * from test.dbo.employees
19 test ; 23 dbo ; 31 employees