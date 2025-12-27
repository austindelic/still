local formula = dofile("examples/example.lua")

-- quick & readable dump
for k, v in pairs(formula.package) do
	print(k, v)
end
